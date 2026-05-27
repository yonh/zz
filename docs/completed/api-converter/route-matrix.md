# Route Matrix

> Phase P0 deliverable. 定义路由前缀语义、当前与未来支持矩阵、与 `proxy_handler` / `rewriter` 的职责边界。

---

## 1. 路由前缀总览

| 前缀 | 语义 | source → target | 阶段 |
|---|---|---|---|
| `/v1/*` | 透明代理（不变） | — | 现有 |
| `/a2o/v1/*` | Anthropic → OpenAI Chat | Anthropic → OpenAIChat | P4（首版） |
| `/o2a/v1/*` | OpenAI Chat → Anthropic | OpenAIChat → Anthropic | P4（首版） |
| `/a2r/v1/*` | Anthropic → OpenAI Responses | Anthropic → OpenAIResponses | 未来 |
| `/r2a/v1/*` | OpenAI Responses → Anthropic | OpenAIResponses → Anthropic | 未来 |
| `/anthropic/v1/*` | 直接使用 Anthropic 格式（显式标注） | Anthropic → Anthropic | 未来（语义=透明 + 强制 api_type=anthropic） |
| `/openai/v1/*` | 直接使用 OpenAI Chat 格式 | OpenAIChat → OpenAIChat | 未来 |
| `/responses/v1/*` | 直接使用 OpenAI Responses 格式 | OpenAIResponses → OpenAIResponses | 未来 |

**前缀命名规则：** `<source-code><2><target-code>` 全小写。Anthropic=a，OpenAI Chat=o，OpenAI Responses=r，Gemini=g（保留）。同源同目标用全名前缀以避免歧义。

---

## 2. 入站路径 → 上游路径

| 入站 | source | target | 上游 path | 备注 |
|---|---|---|---|---|
| `/a2o/v1/messages` | Anthropic | OpenAIChat | `/v1/chat/completions` | 首版 |
| `/o2a/v1/chat/completions` | OpenAIChat | Anthropic | `/v1/messages` | 首版 |
| `/a2r/v1/messages` | Anthropic | OpenAIResponses | `/v1/responses` | 未来 |
| `/r2a/v1/responses` | OpenAIResponses | Anthropic | `/v1/messages` | 未来 |
| `/anthropic/v1/messages` | Anthropic | Anthropic | `/v1/messages` | 未来 |
| `/openai/v1/chat/completions` | OpenAIChat | OpenAIChat | `/v1/chat/completions` | 未来 |
| `/responses/v1/responses` | OpenAIResponses | OpenAIResponses | `/v1/responses` | 未来 |

`converter::target_path(source, target, inbound_path)` 是唯一的入站→上游路径映射函数。其它非 `messages`/`chat/completions`/`responses` 的子路径（例如 `/v1/models`）：

- 在转换前缀下：返回 `Err(unsupported_feature)`，由 P6 降级返回 405/404 错误体。
- 在 `/v1/*`：保持透明代理。

Query string 与片段（fragment）原样转发。

---

## 3. 路由分发流程（main.rs）

```
incoming path
  ├─ starts_with "/v1/"             → proxy::proxy_handler (透明，不变)
  ├─ starts_with "/a2o/v1/"         → conversion_proxy_handler(Anthropic, OpenAIChat)
  ├─ starts_with "/o2a/v1/"         → conversion_proxy_handler(OpenAIChat, Anthropic)
  ├─ starts_with "/a2r/v1/"         → 未实现：返回 501 + JSON 错误（首版）
  ├─ starts_with "/r2a/v1/"         → 同上
  ├─ starts_with "/anthropic/v1/"   → 同上（首版）
  ├─ starts_with "/openai/v1/"      → 同上
  ├─ starts_with "/responses/v1/"   → 同上
  └─ 其它（admin/UI/ws）             → 现有路由（不变）
```

**严格匹配顺序：** 必须按上表顺序，避免 `/v1/` 先于 `/a2o/v1/` 命中。建议使用 axum 路由 `or` 组合，或在统一中间件里 `match`。

---

## 4. 职责边界

| 模块 | 职责 | 不做 |
|---|---|---|
| `main.rs` 路由层 | 前缀匹配、分发到 handler、注入 `(source, target)` | 不解析 body、不改 URL |
| `proxy::conversion_proxy_handler` | 调度：read body → convert_request → 上游请求 → convert_response → 写回 | 不直接做字段映射 |
| `converter` | 纯函数：body / SSE 转换；提供 `target_path()` | 不做网络 IO、不读 provider 配置 |
| `rewriter` | host / Authorization / 现有 header 注入；接收 converter 已重写的路径 | 不再做协议级路径分支 |
| `provider` | provider 选择、`api_type` 标注 | 不感知前缀 |
| `config` | 解析 `api_type`、`enable_conversion_fallback`、`conversion_log_level` | — |

**关键约束：** `rewriter` 不再硬编码 `/v1/messages` ↔ `/v1/chat/completions` 的转换。所有协议级路径决策集中在 `converter::target_path()`。

---

## 5. Provider 选择策略

转换前缀下的 provider 匹配：

1. 由 `target` 决定上游应支持的 `api_type`：
   - target=OpenAIChat ⇒ 仅匹配 `api_type ∈ {"openai-chat","auto"}` 的 provider。
   - target=Anthropic ⇒ 仅匹配 `api_type ∈ {"anthropic","auto"}` 的 provider。
2. 如无匹配 provider：返回 502 + 错误体 `no_matching_provider_for_target_api`。
3. `/v1/*` 透明代理保持现有 provider 选择不变（按 model 等条件）。

`auto` 在首版按 OpenAIChat 处理（与 `field-mapping.md` 一致）。

---

## 6. 兼容性与回归

- `/v1/*` 字节级与现状一致（请求/响应/流式）。
- 未启用任何 `/aXo/` 前缀时，`converter` 模块代码可被链接但路径不会进入。
- 配置默认值保证旧 `config.toml` 加载行为不变。
- 新增响应头（`X-Conversion-*`）仅出现在转换前缀路径上，不污染 `/v1/*`。

---

## 7. 未来扩展指南（写入 `docs/dev/api-converter.md`）

新增一个前缀的步骤：

1. 在 `ApiType` 中确认 source/target 已存在，否则补枚举。
2. 在本文件 §1、§2 增加表项。
3. `converter` 中实现对应 `Converter` 结构体（`request` + `response` + 可选 `stream`）。
4. `target_path()` 增加分支。
5. `main.rs` 路由分发追加前缀（必须放在 `/v1/` 匹配之前）。
6. provider 选择策略 §5 中登记目标 api_type 约束。
7. 增加单测 + 集成测试 + 手动验收脚本一行。
