# Error Model

> Phase P0 deliverable. 定义 `ConversionError` 语义、`field_path` 命名、日志规范、降级行为。

---

## 1. 数据结构

```rust
#[derive(Debug, Clone)]
pub struct ConversionError {
    /// 简短错误码 + 人读说明，例如 "missing_field: messages"
    pub message: String,
    /// 出错字段路径，使用点分 + 数组下标，例如
    ///   "request.messages[2].content[0].type"
    ///   "response.choices[0].message.tool_calls[1].function.arguments"
    pub field_path: Option<String>,
    /// 出错字段的原始 JSON 值（截断到 1KB，超出截断后带 "...<truncated>"）
    pub original_value: Option<serde_json::Value>,
    /// 完整原始 body（截断到 4KB），便于事后复盘
    pub original_body: Option<bytes::Bytes>,
    /// 错误分类（用于响应头 X-Conversion-Error 与日志聚合）
    pub kind: ConversionErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionErrorKind {
    InvalidJson,        // body 不是合法 JSON
    SchemaMismatch,     // JSON 合法但缺关键字段或类型错
    UnsupportedFeature, // 已识别但当前不支持的特性（如 audio）
    StreamProtocol,     // SSE 解析失败
    Internal,           // converter 自身 bug（断言失败等）
    NotImplemented,     // 桩实现
}
```

### `field_path` 命名规范

- 顶层前缀固定：`request.` 或 `response.` 或 `stream.event[<n>].`。
- 数组用 `[i]`（0 起）。
- 不使用空格、不带引号；键名按原 JSON key 大小写。
- SSE 事件计数从首个 chunk 起累加；`stream.event[12].data.choices[0].delta`。
- 无法定位字段时省略 `field_path`（None）。

---

## 2. 错误码（短码）

短码用于 `X-Conversion-Error` 响应头与日志检索：

| 短码 | kind | 触发条件 |
|---|---|---|
| `invalid_json` | InvalidJson | `serde_json::from_slice` 失败 |
| `missing_field` | SchemaMismatch | 必需字段缺失（如请求无 `messages`） |
| `bad_type` | SchemaMismatch | 字段存在但类型不符 |
| `unsupported_block` | UnsupportedFeature | content block 类型当前不支持 |
| `unsupported_feature` | UnsupportedFeature | 顶层特性不支持（如 audio） |
| `tool_args_invalid_json` | SchemaMismatch | `tool_calls.function.arguments` 非合法 JSON |
| `sse_parse` | StreamProtocol | SSE 行/事件格式错误 |
| `sse_state` | StreamProtocol | 状态机异常（如收到 delta 前未 start） |
| `not_implemented` | NotImplemented | P1 占位实现 |
| `internal` | Internal | 不应发生的分支 |

---

## 3. 日志规范

### 3.1 统一前缀与字段

所有 converter 相关日志使用 `tracing` 并统一在 target/前缀 `[CONVERSION]`，结构化字段：

| 字段 | 必填 | 说明 |
|---|---|---|
| `req_id` | ✓ | 复用现有请求 ID（`proxy.rs` 已有） |
| `route` | ✓ | 入站路径，如 `/a2o/v1/messages` |
| `source` | ✓ | `ApiType` 枚举字符串 |
| `target` | ✓ | 同上 |
| `phase` | ✓ | `request` / `response` / `stream` |
| `status` | ✓ | `start` / `success` / `failed` / `fallback` |
| `field_path` | 视情况 | 错误或 skip 时填 |
| `field_mapped` | 视情况 | 字段成功映射记录 |
| `field_skipped` | 视情况 | 字段被跳过 |
| `error_code` | 失败时 | 见第 2 节短码 |
| `body_preview` | 调试 | 截断到 4KB |
| `latency_ms` | success 时 | 转换耗时 |

### 3.2 日志级别

由 `conversion_log_level` 配置控制（默认 `info`）：

| 事件 | 级别 |
|---|---|
| 转换开始/结束（success） | `info` |
| 字段成功映射枚举 | `debug` |
| 字段 skip / 未知字段 | `warn` |
| 非致命异常（如 unsupported_block 但已降级） | `warn` |
| 致命错误 + 降级触发 | `error` |
| 上游响应原文（成功/失败均输出截断预览） | `debug`（默认）；`failed` 时强制 `error` |

### 3.3 关键事件示例

```
INFO  [CONVERSION] phase=request status=start  source=Anthropic target=OpenAIChat route=/a2o/v1/messages req_id=…
DEBUG [CONVERSION] phase=request field_mapped=system->messages[0]
WARN  [CONVERSION] phase=request field_skipped=top_k path=request.top_k reason=unsupported_in_target
INFO  [CONVERSION] phase=request status=success latency_ms=2
ERROR [CONVERSION] phase=response status=failed error_code=tool_args_invalid_json field_path=response.choices[0].message.tool_calls[0].function.arguments
WARN  [CONVERSION] phase=response status=fallback (returning original body)
```

---

## 4. 降级（Fallback）行为

**触发条件：** `convert_request` 或 `convert_response` 返回 `Err(ConversionError)`，且 `provider.enable_conversion_fallback != false`（默认开启）。

**请求侧降级：**
- 请求体转换失败 ⇒ 不发送任何上游请求，直接返回 `502 Bad Gateway`，body 为 Anthropic 错误体（若入站为 a2o）或 OpenAI 错误体（若入站为 o2a）。
  - 原因：原始请求体格式与上游 provider 期望格式不符，转发会造成无意义的远端错误且消耗配额。
- 响应头：
  - `X-Conversion-Status: failed`
  - `X-Conversion-Error: <短码>`
  - `X-Conversion-Phase: request`

**响应侧降级（核心降级路径）：**
- 上游响应已收到，转换失败 ⇒ 原样转发上游 body 与状态码。
- 响应头追加：
  - `X-Conversion-Status: failed`
  - `X-Conversion-Error: <短码>`
  - `X-Conversion-Phase: response`
- 客户端会拿到上游原生格式（与请求侧期望不一致），需通过文档明确告知客户端解析容错。

**流式响应降级：**
- 流式中途失败：尽量保留已发送事件；后续无法再修复时立即结束流，并在 trailers / 末尾事件中尝试携带错误（若客户端不支持 trailers，仅日志记录）。
- 若 `enable_conversion_fallback=false`：流式失败时强制断开连接（`reset stream`）。

**成功路径头部：**
- `X-Conversion-Status: success`
- `X-Conversion-Source: <ApiType>`
- `X-Conversion-Target: <ApiType>`

---

## 5. 截断策略

| 内容 | 上限 | 标记 |
|---|---|---|
| `original_body` | 4 KiB | 末尾追加 `<...truncated N bytes>` |
| `original_value` (JSON) | 1 KiB（序列化后） | 同上 |
| 日志 `body_preview` | 4 KiB | 同上 |
| SSE 单事件预览 | 1 KiB | 同上 |

截断必须在字符边界（UTF-8 安全），优先按字节截断后向前回退到合法 UTF-8 边界。

---

## 6. 错误响应体格式

入站为 `/a2o/*` 时返回 Anthropic 错误体：
```json
{"type":"error","error":{"type":"api_error","message":"conversion failed: <short code>"}}
```

入站为 `/o2a/*` 时返回 OpenAI 错误体：
```json
{"error":{"message":"conversion failed: <short code>","type":"api_error","code":"<short code>"}}
```

仅在 P4「请求侧降级」与流式不可恢复错误中由 converter 生成；响应侧降级永远透传上游原文。

---

## 7. 测试约束

- 每个短码至少有一条单测覆盖。
- 每个降级分支有集成测试断言：状态码、`X-Conversion-Status` 头、body 内容。
- 截断逻辑单测覆盖 UTF-8 边界（含 4 字节 emoji 跨边界）。
