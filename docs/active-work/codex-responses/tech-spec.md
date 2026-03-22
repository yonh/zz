---
status: active
horizon: current
workflow_stage: breakdown
next_command: /sync-active-work-with-code
last_reviewed: 2026-03-22
---

# ZZ - OpenAI Responses API 技术规格

## 版本：3.1.0
## 状态：Active
## 创建时间：2026-03-22
## 最后更新：2026-03-22

---

## 1. 技术原则

ZZ 必须保持 **body-transparent proxy** 的架构原则。当前 Responses API 交付应作为一组小修复落在现有请求/响应转发路径上，而不是借机引入新的 handler、协议转换层或 provider 能力矩阵。

## 2. 当前代码现实

### 2.1 已经足够的部分

- `proxy.rs` 已经按路径转发请求
- `rewriter.rs` 已经按路径无关方式重写 URL 与 headers
- `extract_model()` 已可用于 Responses 请求，因为 `model` 仍位于顶层
- `extract_token_usage()` 已支持 `input_tokens` / `output_tokens` 风格的非流式响应
- retry 与 failover 仍然是状态码驱动，不依赖协议转换

### 2.2 当前批次缺失的部分

#### A. streaming 请求识别
当前检测只看 `Accept: text/event-stream`，必须补充对请求体 `"stream": true` 的检查。

#### B. API 类型日志字段
当前 `LogEntry` 没有记录请求属于 `chat`、`responses` 还是 `other`。

#### C. Responses token usage 验证
需要用显式测试覆盖 Responses 非流式 payload 的 token 提取行为。

## 3. 当前批次必要变更

### 3.1 `src/stream.rs`
新增一个 helper，用于同时检查：
1. `Accept` header
2. 请求体中的 `"stream": true`

### 3.2 `src/proxy.rs`
- 在最终 streaming 判断前先收集 body bytes
- 调用新的 streaming 检测 helper
- 根据请求 path 推导 `api_type`
- 在日志创建时附加 `api_type`

### 3.3 `src/stats.rs`
扩展 `LogEntry`：

```rust
pub api_type: String
```

### 3.4 测试
新增或更新以下测试：
- 从 header 检测 streaming
- 从 body 检测 streaming
- `detect_api_type()`
- Responses 非流式 token usage 提取

## 4. 建议 helper 逻辑

```rust
fn detect_api_type(path: &str) -> &'static str {
    if path.starts_with("/v1/chat/") {
        "chat"
    } else if path.starts_with("/v1/responses") {
        "responses"
    } else {
        "other"
    }
}
```

## 5. 验证计划

### 单元测试
- body 中带 `"stream": true` 的请求会被识别为 streaming
- body 中没有 `stream` 时，除非 header 告知，否则不视为 streaming
- `/v1/responses` 会被标记为 `responses`
- Responses 非流式 `usage` 能被正确解析

### 手工验证
- 通过 ZZ 向 OpenAI 发送非流式 `POST /v1/responses`
- 通过 ZZ 向 OpenAI 发送 streaming `POST /v1/responses`
- 发送既有 `POST /v1/chat/completions` 作为回归验证

## 6. 当前批次涉及文件

| 文件 | 变更 |
|------|------|
| `src/stream.rs` | 新增检查 body 的 streaming 检测 helper |
| `src/proxy.rs` | 调整 streaming 判断顺序，并补 `api_type` 日志 |
| `src/stats.rs` | 给 `LogEntry` 增加 `api_type` |
| `src/proxy.rs` 测试 | 验证 Responses token usage 提取 |

## 7. 明确延期

以下内容明确不属于当前技术规格，统一保留在 roadmap 文档中：
- UI、统计与 WebSocket 的观测增强
- path-based routing 的 schema 扩展
- Responses-to-Chat adapter 设计
