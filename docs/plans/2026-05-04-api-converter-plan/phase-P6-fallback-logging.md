# Phase P6 — 降级机制 + 响应头标记 + 日志规范化

**Depends on:** P4
**Type:** hardening
**Goal:** 把 P4/P5 中临时的错误处理与日志收敛到 `error-model.md` 定义的规范形态。

---

## Scope

### 1. 响应头规范

成功路径（非流 + 流）：
- `X-Conversion-Status: success`
- `X-Conversion-Source: <ApiType>`
- `X-Conversion-Target: <ApiType>`

失败路径：
- `X-Conversion-Status: failed`
- `X-Conversion-Phase: request | response | stream`
- `X-Conversion-Error: <短码>`
- 不泄露 `field_path` 给客户端（仅日志）。

### 2. 降级行为统一

按 `error-model.md` §4：
- 请求侧失败：502 + 错误体（按入站前缀决定形态）。
- 响应侧失败：透传上游 body + 状态码 + 失败响应头。
- 流式失败：尽量优雅收尾（已开始）或透传（未开始）。
- `provider.enable_conversion_fallback=false` 时：响应侧失败也直接 502；流式失败强制 reset。

### 3. 日志规范化

- 统一 tracing target：`zz::conversion`（或全局 `[CONVERSION]` 前缀）。
- 必填字段：`req_id, route, source, target, phase, status`。
- 字段映射 enumerate 在 `debug` 级；skip 在 `warn` 级；致命错误在 `error` 级。
- `body_preview` 用 `error-model.md` §5 的 UTF-8 安全截断工具。
- `latency_ms` 在 success 时输出。

### 4. 错误体生成器

提供 `converter::error_body(target_inbound: ApiType, code: &str) -> Bytes`：
- 入站 a2o → Anthropic 错误体。
- 入站 o2a → OpenAI 错误体。

## Files Touched

- `src/converter/error.rs`（截断工具、`ConversionErrorKind::short_code()`、`error_body()`）
- `src/converter/logging.rs`（新增：日志辅助宏 / 函数）
- `src/proxy.rs`（替换 P4 临时实现为统一调用）
- `tests/converter_truncate.rs`（UTF-8 边界测试，含 4 字节 emoji 跨边界）
- `tests/integration_fallback_request.rs`
- `tests/integration_fallback_response.rs`
- `tests/integration_fallback_stream.rs`

## Acceptance Criteria

- 故意构造每类错误（每个短码至少一条），断言：状态码、`X-Conversion-Status/Phase/Error`、body 形态。
- 截断单测覆盖：
  - 4097 字节纯 ASCII（截到 4096）。
  - 4 字节 emoji 跨 4096 边界（回退到 4092）。
  - 1KiB JSON 序列化截断保持合法 UTF-8。
- `enable_conversion_fallback=false` 路径单测覆盖。
- 日志快照测试（可用 `tracing-test`）覆盖 success / fallback / 字段 skip 三类典型事件。

## Non-Goals

- 不改变字段映射规则。
- 不引入新协议路径。
