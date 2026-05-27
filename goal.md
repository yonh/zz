# 开发目标：OpenAI Responses ↔ Chat Completion 协议转换

> 最后更新: 2026-05-27 | 审核轮次: 多角色审核 (架构/实现/安全/Codex兼容性)
> 审核参与方: 4 路并行 Agent (架构师、实现工程师、安全专家、Codex集成专家)
> 测试框架: TDD 3-Level 路径图 (见下方)

---

## 问题

新版 Codex 只支持 OpenAI **Responses API** (`wire_api = "responses"`)，发送 `POST {base_url}/responses` 格式请求。但 zz 作为反向代理上游的许多 LLM 提供商只支持 **Chat Completion API** (`POST /v1/chat/completions`)，导致无法接入 Codex。

## 目标

在 zz 中实现 **Responses API ↔ Chat Completion API** 双向协议转换，使 Codex 能通过 zz 代理接入仅支持 Chat API 的提供商。

接入拓扑：

```
Codex (Responses API) → zz (协议转换) → 上游提供商 (Chat Completion API)
```

---

## 架构

### 路由前缀

| 前缀 | Codex 实际发送路径 | zz 目标路径 | 方向 |
|------|---|---|---|
| `/r2c` | `POST /r2c/responses` | `POST /v1/chat/completions` | Responses → Chat (主场景) |
| `/c2r` | `POST /c2r/chat/completions` | `POST /v1/responses` | Chat → Responses (对称) |

**重要**: Codex 配置 `base_url = "http://127.0.0.1:9091/r2c"` 后，实际发送的路径是 `POST /r2c/responses` — 不是 `/r2c/v1/responses`。zz 的路由派发需要匹配 `/r2c/`（前缀匹配），然后用 `target_path()` 重写为上游 `/v1/chat/completions`。

### 模块结构

新增两个转换器文件，沿袭现有 `ApiConverter` trait：

```
src/converter/
├── mod.rs                                  # ApiType + ApiConverter trait + target_path()
├── anthropic_to_openai.rs                  # 已有
├── openai_to_anthropic.rs                  # 已有
├── openai_responses_to_chat.rs             # 新增: Responses→Chat (主转换器)
├── openai_chat_to_responses.rs             # 新增: Chat→Responses (对称转换器)
├── stream.rs                               # 需改造: 新增 Responses↔Chat 流状态机
├── telemetry.rs                            # 已有 (可复用)
└── known_fields.rs                         # 需更新: 添加 Responses API 字段
```

### proxy.rs 改造要点 (CRITICAL)

现有 `conversion_proxy_handler` 的 `match source` 只处理了 `Anthropic` 和 `OpenAIChat`，需要新增 `OpenAIResponses` 分支。

响应转换逻辑不能再依赖 `match target { OpenAIChat => AnthropicConverter }` 的隐含假设，必须改用 `(source, target)` 联合派发。

---

## 协议映射

### 请求: Responses → Chat

| Responses API | Chat API | 说明 | 审核标记 |
|---|---|---|---|
| `input` (string) | `messages[0] = {role:"user", content:string}` | 简单字符串输入 | |
| `input[]` (input_items) | `messages[]` | 数组逐个映射 | 注意 `input_item.type` 可能为 `message`/`computer_call`/`computer_call_output`/`reasoning`/`file_search_call`，非 `message` 类型应返回 `UnsupportedFeature`，不可静默忽略 |
| `instructions` | `messages[0]` system 消息 | 插入数组最前面 | 逆向转换时需要注意去重 |
| `model` | `model` | 直接映射 | |
| `max_output_tokens` | `max_tokens` 或 `max_completion_tokens` | 注意默认值行为差异 | |
| `temperature` / `top_p` | 同名字段 | 直接映射 | |
| `tools[]` | `tools[]` | 结构基本一致 | |
| `tool_choice` | `tool_choice` | `"any" == "required"`，其他相同 | |
| `metadata` | `metadata` | 直接映射 | |
| `store` | 忽略 | Chat API 不支持 | |
| `previous_response_id` | **待定 — 需确认 Codex 实际行为** | zz 无状态，如 Codex 跟随完整 `input` 则可丢弃；如仅依赖 ID 则需缓存方案 | **CRITICAL** |

### 响应: Chat → Responses

| Chat API | Responses API | 说明 | 审核标记 |
|---|---|---|---|
| `choices[0].message.content` | `output[0].type:"message" → content[0].type:"output_text" → text` | 双层嵌套 | |
| `choices[0].message.tool_calls` | `output[]` 中 `type:"function_call"` items | Chat API 在 message 内嵌套多个 tool_calls；Responses 在 output 层展开 | 注意多消息映射的有损性 |
| `choices[0].finish_reason` | `output[].stop_reason` | `stop→end_turn`, `length→max_tokens`, `tool_calls→tool_use` | |
| `usage.prompt_tokens` | `usage.input_tokens` | 重命名 | |
| `usage.completion_tokens` | `usage.output_tokens` | 重命名 | |
| 新生成 | `id: "resp_..."` | 必须生成 `resp_` 前缀 ID | **CRITICAL**: 用于 `previous_response_id` 增量请求 |
| 新生成 | `object: "response"` | 固定值 | |
| `created` | `created` | 时间戳 | |
| `model` | `model` | 直接映射 | |

### 辅助端点

| 端点 | Codex 行为 | zz 所需处理 | 优先级 |
|---|---|---|---|
| `GET /models` | 启动时调用，期望 Codex 自定义格式 | 需要拦截并构造兼容响应（含 `slug`, `display_name`, `default_reasoning_level` 等字段）或透传转换 | **CRITICAL** |
| `GET /responses/{id}` | 获取单个 response | 可选实现，非流式场景可能需要 | MEDIUM |
| `POST /responses/{id}/input_items` | 追加 input items | 可选实现 | LOW |

---

## 流式 SSE 转换

### Codex 期望的 SSE 事件 (审核发现)

原始 goal.md 写的是 `response.done`，但 Codex 实际期待 **`response.completed`**。此外还需要支持：

| 事件 | 必需性 | 说明 |
|---|---|---|
| `response.output_text.delta` | CRITICAL | 文本增量 |
| `response.output_text.done` | CRITICAL | 文本片段完成 |
| `response.completed` | CRITICAL | 全响应完成。**不是 `response.done`** |
| `response.failed` | HIGH | 错误事件 |
| `response.cancelled` | MEDIUM | 取消事件 |
| `response.output_item.done` | HIGH | 每个 output item 完成 |
| `reasoning.encrypted_content` | LOW | 推理 token 流 |

Chat API SSE 格式（无事件名，`data: {"choices":[{"index":0,"delta":{}}]}`）与 Responses API SSE（命名事件，多层结构）差异极大。现有 `StreamConverter` 状态机（`OAToAnState` / `AnToOAState`）完全不相容，需要新增专门的状态机。

### 流式转换架构决策

现有 `StreamConverter` 使用 `enum StreamState { OAToAnState(...), AnToOAState(...) }` 硬编码状态机。新增 Responses↔Chat 后，建议改为更通用的派发机制：

```
StreamState 扩展为:
enum StreamState {
    AnToOA(AnToOAState),
    OAToAn(OAToAnState),
    ResponsesToChat(ResponsesToChatState),  // 新增
    ChatToResponses(ChatToResponsesState),  // 新增
}
```

**Phase 4 前必须先抓包确认 Codex 实际 SSE 事件序列**，具体方法：在测试终端窗口中启动 tcpdump/wireshark 或使用中间人代理记录 Codex → smai 的请求。

---

## Headers 处理 (审核发现)

Codex 会发送以下 headers，其中部分需要在转换时剥离/修改：

| Header | 处理方式 |
|---|---|
| `Authorization: Bearer <zz-api-key>` | 替换为上游 provider 的 API key (zz 现有机制) |
| `OpenAI-Beta: responses;sw=2026-02-06` | **需剥离**，Chat API 提供商不认识此 header |
| `x-openai-subagent` | **需剥离** |
| `x-openai-memgen-request` | **需剥离** |
| `x-codex-installation-id` | 可选保留或剥离 |
| `x-client-request-id` | 可选保留 |
| `Content-Type: application/json` | 保留 |
| `Accept: text/event-stream` (流式) | 保留 |

---

## 实施阶段 (修正版)

### Phase 1: 基础框架与路径注册 (0.5天)

- [ ] 在 `converter.rs` 的 `target_path()` 添加路径映射:
  - `(OpenAIResponses, OpenAIChat, "/r2c/responses")` → `"/v1/chat/completions"`
  - `(OpenAIChat, OpenAIResponses, "/c2r/chat/completions")` → `"/v1/responses"`
- [ ] 在 `main.rs` 路由派发中**在普通 proxy 之前**添加 `/r2c/` 和 `/c2r/` 路由（注意优先级：必须在 `/responses/` 的 501 fallback 之前）
- [ ] 在 `proxy.rs` 的 `conversion_proxy_handler` 中新增 `ApiType::OpenAIResponses` 作为 source 的分支
- [ ] 修复响应转换逻辑：用 `(source, target)` 联合派发替代现有 `match target` 的隐含假设
- [ ] 新增空转换器桩文件: `openai_responses_to_chat.rs`, `openai_chat_to_responses.rs`
- [ ] 在 `provider.rs` 中确保 `select_for_target` 对 `OpenAIResponses` target 的匹配逻辑正确
- [ ] 添加 `known_fields.rs` 中 Responses API 字段列表
- [ ] 建立集成测试框架

### Phase 2: `/r2c/` 主路径完整实现 (3-4天)

按路由方向（而非按文件）划分，使本阶段可独立测试：

- [ ] 实现 `OpenAIResponsesToChatConverter` (全部在 `openai_responses_to_chat.rs`):
  - `convert_request`: Responses → Chat (input, instructions, tools, tool_choice 等)
  - `convert_response`: Chat → Responses (choices → output, usage, 错误体转换)
  - 处理所有 input_item 类型（`message` 以外返回 UnsupportedFeature）
  - 处理 `previous_response_id`（根据 Codex 实际行为决断：丢弃或缓存）
- [ ] 单元测试: 覆盖率 80%+
- [ ] Headers 清理：在转换过程中剥离 `OpenAI-Beta` 等 Chat API 不认识的 headers
- [ ] 错误体双向转换

### Phase 3: `/c2r/` 对称路径实现 (3-4天)

如无部署需求可推迟：

- [ ] 实现 `OpenAIChatToResponsesConverter` (全部在 `openai_chat_to_responses.rs`):
  - `convert_request`: Chat → Responses (messages → input, system → instructions)
  - `convert_response`: Responses → Chat (output → choices)
  - `instructions` 去重逻辑
  - 生成 `resp_` 前缀 ID
- [ ] 单元测试: 覆盖率 80%+

### Phase 4: 流式转换 (5-8天)

- [ ] **前提**: 通过抓包确认 Codex ↔ Responses API 的实际 SSE 事件序列
- [ ] 在 `stream.rs` 中添加 `ResponsesToChatState` 和 `ChatToResponsesState` 两种状态机
- [ ] 实现事件流转换:
  - `response.output_text.delta` ↔ `choices[0].delta.content`
  - `response.completed` → `data: [DONE]` (及反方向)
  - `response.failed` → Chat API 错误 SSE
- [ ] `ConversionStreamBody` 复用（无需修改）
- [ ] 流式单元测试

### Phase 5: 集成测试与 Codex 验证 (2-3天)

- [ ] 在 `/tmp/zz-codex-test/` 中创建测试配置
- [ ] 编写集成测试（启动 zz 实例，HTTP 请求验证）
- [ ] `GET /models` 端点实现或透传方案
- [ ] Codex 接入验证:

```bash
# 启动 zz (独立端口)
./target/debug/zz -c /tmp/zz-codex-test/config.toml -l 127.0.0.1:9091

# 验证方法
curl http://127.0.0.1:9091/r2c/health   # 健康检查

# Codex 配置
codex -c model_providers.zz='{name="codex",base_url="http://127.0.0.1:9091/r2c",wire_api="responses",env_key="ZZ_API_KEY"}'
codex doctor   # 验证连接
```

- [ ] 端到端验证：用 Codex 实际工作流（不仅仅是 ping）做完整测试
- [ ] 编写 Manual Verification 清单

---

## 审核发现的 CRITICAL 问题

以下为多角色审核中发现的、在实施前必须解决的 CRITICAL 问题：

| ID | 来源 | 问题 | 影响 | 解决方式 |
|---|---|---|---|---|
| **C1** | 架构 | `conversion_proxy_handler` 中无 `OpenAIResponses` 分支 | 所有 /r2c/ 请求返回 400 | Phase 1 新增分支 |
| **C2** | 架构 | 响应转换方向错配（现有代码假设 target=OpenAIChat → AnthropicConverter） | 响应转换结果完全错误 | 改为 `(source, target)` 联合派发 |
| **C3** | 架构/实现 | SSE 状态机完全不支持 Responses↔Chat | 流式转换直接 panic | Phase 4 新增状态机 |
| **C4** | Codex | `response.done` 事件名错误 — 实际为 `response.completed` | Codex 永远收不到完成信号 | 修正事件名 |
| **C5** | Codex | Codex 启动时调用 `GET /models` 期望自定义格式 | Codex 无法通过启动检查 | 需要拦截并构造兼容响应 |
| **C6** | 实现 | `previous_response_id` 需要会话状态 | 增量对话无法维持上下文 | 需确认 Codex 是否同时发送完整 `input` |

---

## 安全注意事项 (审核发现)

| 级别 | 问题 | 说明 |
|---|---|---|
| HIGH | SSE 注入风险 | 流式转换中应对透传事件类型做白名单校验，防止 `\n` 注入构造伪造事件 |
| HIGH | 请求体无大小限制 | 在高版本中应限制请求体大小（如 10MB），防止 OOM |
| MEDIUM | `OpenAI-Beta` header 透传 | 上游 Chat API 可能因无法识别此 header 而拒绝请求，需剥离 |
| MEDIUM | `instructions` → system 消息的审核绕过 | 映射后发送到 Chat API 提供商，注意内容审核策略差异 |

---

## 工作量估算 (修正版)

| Phase | 原估算 | 修正估算 | 风险系数 |
|---|---|---|---|
| Phase 1 | 未明确 | 0.5天 | 1.0x |
| Phase 2 | 2-3天 | 3-4天 | 1.5x (input_item 类型覆盖 + 错误转换) |
| Phase 3 | 2-3天 | 3-4天 | 1.5x (instructions 去重 + error mapping) |
| Phase 4 | 未明确 | 5-8天 | 2.0x (SSE 格式不确定性 + StreamConverter 改造) |
| Phase 5 | 1-2天 | 2-3天 | 1.5x (model endpoint + Codex 调试) |
| **合计** | ~8天 | **14-22人天** | |

核心不确定性在于 Phase 4 的 SSE 格式确认和 StreamConverter 改造。

---

## 已知降级选择

1. **Phase 3 可推迟**: 如果只需 Codex 通过 zz 接入 Chat-only 提供商（单向），Phase 3 的 `/c2r/` 不是必需的
2. **`previous_response_id`** 如有状态保存需求，可推迟到二期
3. **`GET /models`** endpoint 可先实现简单版本返回硬编码列表，后续再做动态生成
4. **`input_item` 非 `message` 类型**：先实现 `message` 类型，其余返回 `UnsupportedFeature`，后续逐步扩展

---

*初始版生成: 2026-05-27*
*审核轮次: 架构审核 / 实现审核 / 安全审核 / Codex 兼容性审核 (并行 4 路)*
*修正: Codex 实际路径 (非 /v1/responses)、SSE 事件名 (response.completed 非 response.done)、
  proxy.rs 改造需求 (C1/C2)、Phase 划分修正 (按路由方向)、工作量修正*

---

## 测试驱动路径图 (TDD Roadmap)

测试文件: `tests/responses_chat_conversion.rs` + `tests/common/mod.rs`

### 测试 3-Level 体系

```
Level 0: Converter 单元测试 (纯函数，可直接运行)
├── Phase 2 目标 (17 tests → 全部变绿)
│   ├── A: Responses → Chat 请求转换 (11 个测试)
│   │   ├── 最简单: string input → messages[{role:"user", content}]
│   │   ├── instructions → system message
│   │   ├── input array (developer role)
│   │   ├── max_output_tokens → max_tokens
│   │   ├── tools + tool_choice 映射
│   │   ├── stream 字段保留
│   │   ├── previous_response_id 丢弃
│   │   └── temperature/top_p/metadata/stop 透传
│   └── B: Chat → Responses 响应转换 (6 个测试)
│       ├── 最简单: choices[0].message.content → output[].output_text
│       ├── tool_calls → function_call output
│       ├── finish_reason 映射 (stop→end_turn, length→max_tokens)
│       ├── model/created 透传
│       └── usage 映射 (prompt_tokens→input_tokens)
│
├── Phase 3 目标 (2 tests, 当前 #[ignore])
│   ├── C: Chat → Responses 请求转换
│   └── D: Responses → Chat 响应转换 (对称方向)
│
Level 1: 完整 HTTP 往返测试 (需要 zz + mock server)
├── 当前 #[ignore], Phase 2 + Phase 5 启用
├── Test 1-A: 完整往返 (Responses → zz → mock → zz → Responses)
├── Test 1-B: 工具调用往返
├── Test 1-C: 上游错误传播
└── Test 1-D: GET /r2c/models

Level 2: 流式 SSE 转换 (需要 Phase 4)
├── 当前 #[ignore], Phase 4 启用
├── Test 2-A: Responses SSE event → Chat SSE data
├── Test 2-B: Chat SSE data → Responses SSE event
├── Test 2-C: 完整流式事件序列
└── Test 2-D: 跨 chunk 边界缓冲
```

### TDD 实施步调

**"先实现能用，在实现细节"** 的具体含义:

| 步 | 测试集 | 实现内容 | 验收标准 |
|---|---|---|---|
| 1 | Level 0-A-1 | string input → user message | `test_r2c_simple_string_input` ✅ |
| 2 | L0-A-1~4 | 基础字段映射 | A 组 4 个测试 ✅ |
| 3 | L0-A-1~11 | 全部字段映射 | A 组 11 个测试 ✅ |
| 4 | L0-B-1 | choices → output | `test_c2r_simple_text_response` ✅ |
| 5 | L0-B-1~5 | 全部响应转换 | B 组 5 个测试 ✅ |
| 6 | L0-A+B | Level 0 Phase 2 **全部 17 个测试通过** | `cargo test` 无 FAILED ✅ |
| 7 | Level 1-1A | 启动 zz + mock server 的 E2E | `test_r2c_full_roundtrip_simple` ✅ |
| ... | 逐步扩展 | ... | ... |

### 测试工具链

- `tests/common/mod.rs`: Mock upstream HTTP server + zz 配置生成器 + `post_json` 辅助
- `tests/fixtures/responses_*.json`: Responses API 请求/响应样本
- `#[ignore = "Phase X"]`: 标注尚未实现的测试（编译但跳过运行）

### 当前 TDD 状态 (截至文件创建)

```
cargo test --test responses_chat_conversion
  └─ 17 FAILED (Level 0 stubs, TDD RED phase — as expected)
  └─ 11 ignored (Phase 3/4/5)
  └─ 0 passed
```

开始实施后，随着 converter 实现，测试逐个变绿。