# Phase P4 — 路由分发 + conversion_proxy_handler 接入

**Depends on:** P3
**Type:** integration
**Goal:** 把 P2/P3 转换器接入实际请求链路，`/v1/*` 行为字节级不变。

---

## Scope

### 1. `main.rs` 路由分发

按 `route-matrix.md` §3 严格顺序匹配：
- `/v1/*` → `proxy::proxy_handler`（不变）。
- `/a2o/v1/*` → `proxy::conversion_proxy_handler(req, state, Anthropic, OpenAIChat)`。
- `/o2a/v1/*` → `proxy::conversion_proxy_handler(req, state, OpenAIChat, Anthropic)`。
- `/a2r/*`、`/r2a/*`、`/anthropic/*`、`/openai/*`、`/responses/*` → 返回 `501 not_implemented` 错误体（首版桩）。

### 2. `proxy::conversion_proxy_handler`

复用 `proxy_handler` 内部 building blocks（提取共享子函数：provider 选择、`attempt_request`、重试逻辑）。新增流程：

```
1. read inbound body (full buffer; 流式请求体在首版按完整缓冲处理，POST 正文通常 < 1MB)
2. converter.convert_request(body, target) → new_body | Err
   ├─ Err: 写 X-Conversion-Status=failed, X-Conversion-Phase=request, X-Conversion-Error=<code>
   │       返回 502 + 错误体（按入站前缀决定 Anthropic / OpenAI 错误体）
   └─ Ok: 替换 req body
3. converter.target_path(source, target, inbound_path) → 重写 req URI path
4. provider 选择：限制为目标 api_type 兼容（见 route-matrix.md §5）
5. attempt_request → upstream_resp
6. if upstream_resp.is_stream: 跳到 P5
   else:
       read upstream body
       converter.convert_response(body, source, target, false) → new_body | Err
       ├─ Err: 透传上游 body + 状态码 + X-Conversion-Status=failed/X-Conversion-Phase=response/X-Conversion-Error
       └─ Ok: 写 new_body + X-Conversion-Status=success
```

### 3. `target_path()` 实装

填充 `route-matrix.md` §2 的首版两条映射：
- `(Anthropic, OpenAIChat, "/a2o/v1/messages")` → `/v1/chat/completions`
- `(OpenAIChat, Anthropic, "/o2a/v1/chat/completions")` → `/v1/messages`
- 其它子路径 → `Err(unsupported_feature)`。

### 4. `rewriter.rs` 调整

- 移除（如有）协议级路径硬编码分支。
- 接收 converter 已重写过的 path，只负责 host/Authorization。

## Files Touched

- `src/main.rs`
- `src/proxy.rs`（新增 `conversion_proxy_handler`，提取共享 helpers）
- `src/converter/mod.rs`（`target_path` 实装）
- `src/rewriter.rs`（清理协议级分支）
- `src/provider.rs`（按 target api_type 过滤的辅助函数）
- `tests/integration_conversion_a2o.rs`（新增）
- `tests/integration_conversion_o2a.rs`（新增）
- `tests/integration_v1_passthrough.rs`（回归）

## Acceptance Criteria

- 集成测试（mock 上游 HTTP server）：
  - `/a2o/v1/messages` 发 Anthropic 请求 → mock 上游收到 OpenAI Chat 格式 → 上游返 OpenAI 响应 → 客户端拿到 Anthropic 格式。
  - `/o2a/v1/chat/completions` 反向同理。
  - `/v1/messages` 与 `/v1/chat/completions` 透明代理：请求/响应体字节一致。
  - 无匹配 provider 时返回 502 + `no_matching_provider_for_target_api`。
- `cargo test` 全绿；`cargo clippy -- -D warnings`。
- 启动后访问 admin/UI/ws 现有路由不受影响。

## Non-Goals

- 流式响应转换（P5）。
- 详细日志规范化与响应头形态完善（P6）。
- 配置字段新增（P7，P4 内可临时硬编码默认值）。
