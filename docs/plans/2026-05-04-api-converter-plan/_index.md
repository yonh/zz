# API Converter Implementation Plan

**Goal:** 在 ZZ proxy 中引入显式的协议转换路由前缀（`/a2o/v1/*`、`/o2a/v1/*` 等），实现主流 LLM API 接口规范之间的双向转换，初期聚焦 Anthropic Claude API ↔ OpenAI Chat Completions API。保持 `/v1/*` 现有透明代理行为不变。

**Scope (initial release):**
- 透明代理 `/v1/*` 完全保持不变。
- 新增 `/a2o/v1/*`（Anthropic → OpenAI Chat）与 `/o2a/v1/*`（OpenAI Chat → Anthropic）。
- 转换失败时降级返回原始响应并写入详细日志。

**Out of scope (留给后续阶段):**
- `/a2r/v1/*`、`/r2a/v1/*`（Anthropic ↔ OpenAI Responses）。
- `/anthropic/v1/*`、`/openai/v1/*`、`/responses/v1/*` 直接格式路由。
- Gemini、OpenAI Completions（旧版）转换。
- 多模态、tool_use 复杂结构（首版仅做文本/基本工具调用）。

**Tech Stack:** Rust, axum, hyper, tokio, serde_json, bytes, tracing。

**Reference:**
- 主流 API 规范：Anthropic `/v1/messages`、OpenAI `/v1/chat/completions`、`/v1/completions`、`/v1/responses`、Gemini `generateContent`。
- 相关源文件：`src/main.rs`、`src/proxy.rs`、`src/rewriter.rs`、`src/provider.rs`、`src/config.rs`。

**Design Documents (P0 frozen):**
- [Field Mapping](./field-mapping.md)
- [Error Model](./error-model.md)
- [Route Matrix](./route-matrix.md)

**Phase Specs:**
- [P1 — Skeleton](./phase-P1-skeleton.md)
- [P2 — Request a2o](./phase-P2-request-a2o.md)
- [P3 — Response o2a (non-stream)](./phase-P3-response-o2a.md)
- [P4 — Routing & Handler](./phase-P4-routing-handler.md)
- [P5 — Stream SSE](./phase-P5-stream.md)
- [P6 — Fallback & Logging](./phase-P6-fallback-logging.md)
- [P7 — Config Extension](./phase-P7-config.md)
- [P8 — Verification & Docs](./phase-P8-verification.md)
- [P9 — Iteration Telemetry (增量迭代闭环)](./phase-P9-iteration-telemetry.md)

**Agent 开发提示词：**
- [Dev Prompts (Master + 各阶段)](./dev-prompts.md)

---

## Phase Overview

```yaml
phases:
  - id: P0
    name: "Spec & Field Mapping Freeze"
    type: design
    deliverable: "field-mapping.md, error-model.md, route-matrix.md"
  - id: P1
    name: "Skeleton: ApiType + Converter trait + ConversionError"
    type: foundation
    depends-on: [P0]
  - id: P2
    name: "Anthropic → OpenAIChat 请求体转换"
    type: impl
    depends-on: [P1]
  - id: P3
    name: "OpenAIChat → Anthropic 响应体转换 (non-stream)"
    type: impl
    depends-on: [P2]
  - id: P4
    name: "路由分发 + conversion_proxy_handler 接入"
    type: integration
    depends-on: [P3]
  - id: P5
    name: "流式 SSE 双向转换"
    type: impl
    depends-on: [P4]
  - id: P6
    name: "降级机制 + 响应头标记 + 日志规范化"
    type: hardening
    depends-on: [P4]
  - id: P7
    name: "配置扩展 (api_type / fallback / log level)"
    type: config
    depends-on: [P1]
  - id: P8
    name: "测试矩阵 + 手动验收 + 文档"
    type: verification
    depends-on: [P5, P6, P7, P9]
  - id: P9
    name: "Iteration Telemetry: 主动采集 + 回放 + Admin/UI 暴露"
    type: observability
    depends-on: [P6, P7]
```

**增量迭代理念（贯穿全程）：** 我们承认首版字段映射不可能完备。P9 把"日常使用 → 发现问题 → 修复"做成程序自身具备的能力——converter 主动上报未知字段、按 signature 去重保留脱敏样本、admin API/UI 暴露 top issues、`convert-replay` 工具支持本地回放、回归样本固化进自动化测试。**P2/P3/P5 在实现时必须在每个字段处理点接入 P9 的采集 API（不是事后加日志）**，否则 P9 形同虚设。

---

## Phase P0 — Spec & Field Mapping Freeze（设计冻结）

**目标：** 在写代码前冻结字段映射表与错误模型，避免后续返工。

**Deliverables:**
- `field-mapping.md`：列出 Anthropic ↔ OpenAIChat 每个字段的映射、跳过、默认值。
- `error-model.md`：定义 `ConversionError` 字段语义、`field_path` 命名规范、原始内容截断策略（4KB）。
- `route-matrix.md`：列出当前与未来支持的路由前缀及对应 `(source, target)` 对。

**关键映射点（需在 P0 明确）：**

| 方向 | Anthropic 字段 | OpenAI Chat 字段 | 备注 |
|---|---|---|---|
| req | `model` | `model` | 透传，由 provider 重写决定 |
| req | `messages[]` | `messages[]` | role 映射；content 数组需要展平 |
| req | `system` (string \| array) | `messages[0]` with `role=system` | 多段 system 拼接 |
| req | `max_tokens` | `max_tokens` (非 reasoning 模型) / `max_completion_tokens` | 由 provider api_type 决定 |
| req | `stop_sequences[]` | `stop` | 数组保留 |
| req | `temperature/top_p/top_k` | `temperature/top_p` | `top_k` 跳过并记日志 |
| req | `tools[]` (Anthropic schema) | `tools[]` (OpenAI function schema) | schema 重排：`input_schema` → `parameters` |
| req | `tool_choice` | `tool_choice` | enum 映射 |
| req | `metadata.user_id` | `user` | 透传 |
| req | `anthropic_beta`/`anthropic_version` | — | 跳过+warn |
| req | `stream` | `stream` | 透传 |
| resp | `choices[0].message.content` (string) | `content[].text` (type=text) | 包装为单 block |
| resp | `choices[0].message.tool_calls[]` | `content[].tool_use` | id/name/input 映射 |
| resp | `finish_reason` | `stop_reason` | enum：`stop→end_turn`, `length→max_tokens`, `tool_calls→tool_use` |
| resp | `usage.prompt_tokens` | `usage.input_tokens` | |
| resp | `usage.completion_tokens` | `usage.output_tokens` | |

**SSE 事件映射（P5 详细）：**
- OpenAI: `data: {choices:[{delta:{...}}]}` → Anthropic: `message_start` / `content_block_start` / `content_block_delta` / `content_block_stop` / `message_delta` / `message_stop`。
- 反向同理。

**Exit Criteria:** 三份文档评审通过，关键字段歧义清零。

---

## Phase P1 — Skeleton（最小骨架）

**目标：** 引入数据类型与 trait，但不接入路由。

**Deliverable:** 新增 `src/converter/mod.rs`（或 `src/converter.rs`，按现有风格选择）。

**内容：**
- `pub enum ApiType { Anthropic, OpenAIChat, OpenAICompletions, OpenAIResponses, Gemini, Unknown }`
- `pub trait ApiConverter { fn convert_request(&self, body: &Bytes, target: ApiType) -> Result<Bytes, ConversionError>; fn convert_response(&self, body: &Bytes, source: ApiType, target: ApiType, is_stream: bool) -> Result<Bytes, ConversionError>; }`
- `pub struct ConversionError { message, field_path: Option<String>, original_value: Option<serde_json::Value>, original_body: Option<Bytes> }`
- 占位实现 `AnthropicToOpenAIConverter`（请求/响应均返回 `Err(NotImplemented)`）。
- 单元测试桩：trait dyn 调用、error 构造。

**Exit Criteria:** `cargo build` 通过；`cargo test converter::` 全绿（仅桩测试）。

---

## Phase P2 — Anthropic → OpenAIChat 请求体转换

**目标：** 实现非流式请求体的字段映射。

**Tasks:**
- `system` (string/array) → 注入到 `messages` 头部 system 消息。
- `messages[].content`：string 直通；array of blocks 展平：`text` 拼接、`image` 转 OpenAI vision schema（首版可选择跳过+warn）。
- `tools` schema 重排。
- `max_tokens` 输出键由配置决定。
- 跳过字段（`anthropic_beta`, `anthropic_version`, `top_k`）走 `field_skipped` 日志。
- 未知字段：`tracing::warn` + 继续。

**测试：** 表驱动单测覆盖每个映射规则；含错误用例（`messages` 缺失、`role` 非法）。

**Exit Criteria:** 单测 ≥10 用例全绿。

---

## Phase P3 — OpenAIChat → Anthropic 响应体转换（非流）

**目标：** 把 OpenAI 非流式 `chat.completion` 响应转换为 Anthropic `messages` 响应。

**Tasks:**
- 顶层结构：`id`、`model`、`role=assistant`、`type=message` 包装。
- `choices[0].message.content` → `content: [{type:"text", text:...}]`。
- `tool_calls[]` → `content[]` 中的 `tool_use` block。
- `finish_reason` → `stop_reason` 枚举映射；不识别值记 `field_skipped` 并默认 `end_turn`。
- `usage` 字段映射。

**测试：** 至少 8 用例（纯文本、tool_calls、length、含 refusal 字段、空 content）。

**Exit Criteria:** 单测全绿；返回 JSON 与 Anthropic 官方 schema 字段对齐。

---

## Phase P4 — 路由分发 + conversion_proxy_handler

**目标：** 把转换器接入实际请求链路。

**Tasks:**
- `main.rs` 路由匹配：`/v1/*` 透明；`/a2o/v1/*` → `(Anthropic, OpenAIChat)`；`/o2a/v1/*` → `(OpenAIChat, Anthropic)`。
- 新增 `proxy::conversion_proxy_handler(req, state, source, target)`：
  1. 读取请求体（注意现有 streaming body 处理）。
  2. `convert_request` → 替换 body。
  3. URL 重写：把 `/a2o/v1/messages` 重写为目标路径 `/v1/chat/completions`（由 source/target 决定，集中在 converter 提供 `target_path()`）。
  4. 复用现有 `attempt_request`/provider 选择逻辑。
  5. 非流响应：读取 body → `convert_response` → 替换。
  6. 失败 → 走 P6 降级。
- 不破坏 `/v1/*`：在 `proxy_handler` 之前 dispatch，`/v1/*` 路径完全不进入转换分支。

**测试：**
- 集成测试（mock provider）：`/a2o/v1/messages` 请求，断言上游收到 OpenAI 格式；上游返回 OpenAI 响应，断言客户端收到 Anthropic 格式。
- 回归：`/v1/*` 行为字节级一致。

**Exit Criteria:** 集成测试全绿；`cargo test` 全绿。

---

## Phase P5 — 流式 SSE 双向转换

**目标：** 支持 `stream: true`。

**Tasks:**
- 引入 `StreamConverter`：增量解析上游 SSE，逐事件输出目标格式 SSE。
- 维护转换状态（message_id、当前 content_block index、累计 usage）。
- 处理半包/多事件单 chunk；`[DONE]` 终止；错误事件透传并记录。

**测试：**
- 录制 OpenAI 流式样本 → 转 Anthropic 流式，逐事件断言。
- 反向同理。
- 异常：上游中途断流、非法 JSON。

**Exit Criteria:** 流式单测全绿；手动验收用 curl 流式可看到 Anthropic 事件序列。

---

## Phase P6 — 降级机制 + 响应头 + 日志规范化

**Tasks:**
- 转换失败：原样转发原始响应；返回 `X-Conversion-Status: failed`，附 `X-Conversion-Error: <短码>`。
- 成功：`X-Conversion-Status: success`。
- 日志统一前缀 `[CONVERSION]`，字段：`source`, `target`, `route`, `req_id`, `field_mapped`, `field_skipped`, `error`, `field_path`。
- 请求/响应体截断 4KB；`conversion_log_level` 控制采样级别。
- 失败必须记录 `original_body`（截断）以便复盘。

**Exit Criteria:** 故意构造错误 payload，能在日志中定位字段；客户端始终拿到可用响应（即使是原始格式）。

---

## Phase P7 — 配置扩展

**`ProviderConfig` 增加：**
```
#[serde(default)] pub api_type: String,                    // "anthropic"|"openai-chat"|"openai-responses"|"auto"
#[serde(default)] pub enable_conversion_fallback: bool,    // 默认 true
```

**全局/section 配置：**
```
#[serde(default)] pub conversion_log_level: String,        // "debug"|"info"|"warn"
```

**Tasks:**
- `config.rs` 增字段并保持向后兼容（默认值 = 当前行为）。
- `auto` 含义：根据上游响应 Content-Type / schema 推断（首版可不实现，置 warn + 退回 openai-chat）。
- `admin_api.rs`：暴露字段读写（只读首版亦可）。
- `config.toml.example` 增示例与注释。

**Exit Criteria:** 旧配置文件加载零变更；新字段单测覆盖默认值与解析。

---

## Phase P8 — 测试矩阵 + 手动验收 + 文档

**自动化：**
- `cargo test` 全绿（unit + integration）。
- `cargo clippy -- -D warnings`。
- `cargo build --release`。

**手动验收脚本（写入 `docs/active-work/api-converter/manual-acceptance.md`）：**
1. 配置 OpenAI 类型 provider（如 SenseNova），`api_type="openai-chat"`。
2. 用 Anthropic 格式请求 `/a2o/v1/messages` 非流，验证响应为 Anthropic schema。
3. 同上但 `stream:true`。
4. 故意发送含 `anthropic_beta`、`top_k`、未知字段的请求，验证日志含 `field_skipped` 且响应正常。
5. 故意构造非法 JSON / 缺 `messages` 请求，验证降级 + `X-Conversion-Status: failed`。
6. `/v1/*` 走 Claude 原生 provider，确认零回归。
7. `/o2a/v1/chat/completions` 反向链路同样四步验证。

**文档：**
- `docs/dev/api-converter.md`：架构、路由前缀语义、字段映射、扩展指南（如何加 `/a2r/*`）。
- README 增加协议转换章节链接。

**Exit Criteria:** 所有手动验收勾选；文档合并；功能可发布。

---

## Risks & Mitigations

| 风险 | 对策 |
|---|---|
| 字段映射不完备导致客户端报错 | P6 降级 + `X-Conversion-Status` + 详细日志收集回归 |
| 流式状态机复杂易错 | P5 单独阶段，录制真实样本回放测试 |
| 与 `rewriter.rs` URL 重写耦合 | converter 提供 `target_path()`，rewriter 仅负责 host/auth |
| `auto` api_type 推断歧义 | 首版不实现，明确文档约束 |
| body 大小/性能影响 | 仅转换分支读取整 body；`/v1/*` 流式路径不受影响 |

---

## Non-Goals (明确不做)

- 不修改 `/v1/*` 的现有透明代理逻辑。
- 不一次性实现所有规范，先 Anthropic ↔ OpenAIChat。
- 不因转换失败拒绝请求。
- 不破坏向后兼容（配置/路由/响应行为）。
