---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 007：流模块 - SSE 支持

## 目标

实现 SSE（Server-Sent Events）流式处理，要求零缓冲并支持 chunked transfer。

## BDD 场景

```gherkin
Scenario: Detect SSE request from Accept header
  Given request header Accept = "text/event-stream"
  When is_sse_request() is called
  Then returns true

Scenario: Detect SSE request from Content-Type
  Given request header Content-Type = "application/json"
  And request body contains "stream": true
  When is_sse_request() is called
  Then returns true (parse minimal JSON to check stream field)

Scenario: Pipe SSE chunks in real-time
  Given upstream sends SSE event chunks
  When stream_response() pipes data to client
  Then client receives each chunk immediately
  And no buffering occurs

Scenario: Handle chunked transfer encoding
  Given upstream response uses chunked encoding
  When stream_response() processes it
  Then chunks are forwarded with proper encoding
  And client can parse SSE events correctly

Scenario: Don't retry on mid-stream failure
  Given SSE streaming is in progress
  And upstream connection drops mid-stream
  When error occurs
  Then proxy returns error to client
  And does NOT retry on next provider (streaming already started)
```

## 涉及文件

**创建**：
- `src/stream.rs` - 完整实现

## 历史实施步骤

1. 实现 SSE 检测：
   - `is_sse_request(req: &Request<Body>) -> bool`
   - 检查 `Accept: text/event-stream`
   - 如有需要，最小化解析请求体 JSON，检查 `stream: true`

2. 实现流式透传：
   - 使用 `hyper_util::rt::TokioIo` 封装流
   - 使用 `tokio::io::copy_bidirectional` 或手工读写 chunk
   - 收到 chunk 后立即转发，不做缓冲
   - 保持 chunk 边界

3. 实现 chunked transfer 支持：
   - 在响应上设置 `Transfer-Encoding: chunked`
   - 直接把上游 chunk 转发到下游
   - 兼容 SSE 事件格式（`data:`、`event:`、`id:`、`retry:`）

4. 实现 stream 代理函数：
   - `proxy_stream(upstream: Response<Incoming>, downstream: &mut Response<Full<Bytes>>) -> Result<()>`
   - 必要时支持双向流

## 历史验证方式

运行：

```bash
cargo build
```

手工测试（需先运行服务）：

```bash
curl -N -H "Accept: text/event-stream" http://localhost:9090/v1/chat/completions -d '{
  "model": "qwen-plus",
  "messages": [{"role": "user", "content": "hi"}],
  "stream": true
}'
```

预期：
- 事件流实时出现，没有明显缓冲

## 依赖
- 任务 006（用于处理流中失败的错误逻辑）
