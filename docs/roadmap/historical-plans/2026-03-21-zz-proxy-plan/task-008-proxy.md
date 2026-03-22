---
status: deferred
horizon: long_term
workflow_stage: archived
next_command: /route-task-by-status
last_reviewed: 2026-03-22
---

# 任务 008：代理模块 - 请求/响应转发

## 目标

实现核心代理处理器，将请求转发到上游 provider，并支持 failover 与 retry 逻辑。

## BDD 场景

```gherkin
Scenario: Forward request to selected provider
  Given client sends POST /v1/chat/completions
  And router selects ali-account-1 provider
  When proxy_request() is called
  Then request is forwarded to ali-account-1 base_url
  And Authorization header is rewritten with ali-account-1 api_key
  And response from upstream is returned to client

Scenario: Retry on quota error with next provider
  Given ali-account-1 returns HTTP 429
  When proxy_request() detects quota error
  Then ali-account-1 is marked as cooldown
  And request is retried with next available provider
  And client receives response from second provider

Scenario: Return last error when all providers exhausted
  Given all providers return quota errors
  When proxy_request() tries all providers
  Then returns last provider's error to client
  And status code is 429 (or whatever last error was)

Scenario: Don't retry on client errors
  Given provider returns HTTP 400 (bad request)
  When proxy_request() receives response
  Then returns error to client immediately
  And does NOT retry on next provider

Scenario: Stream SSE responses without buffering
  Given request has Accept: text/event-stream
  When proxy_request() processes response
  Then uses stream::proxy_stream() to pipe chunks
  And client receives events in real-time

Scenario: Respect max_retries configuration
  Given max_retries = 3
  And first 3 providers all fail
  When proxy_request() retries
  Then retries exactly 3 times
  And returns error after 3rd failure (doesn't try 4th)
```

## 涉及文件

**创建**：
- `src/proxy.rs` - 完整实现

## 历史实施步骤

1. 创建 HTTP client（带 TLS）：
   - 使用 `hyper_util::client::legacy::Client` + HTTPS connector
   - 根据配置设置超时

2. 实现代理 handler：
   - `proxy_handler(req: Request<Incoming>, state: AppState) -> Result<Response<Full<Bytes>>, Error>`
   - 提取 path 与 headers
   - 调用 `router.select_provider()`
   - 如果没有可用 provider，则返回 503

3. 实现请求转发循环：
   ```rust
   for attempt in 0..max_retries {
       provider = router.select_provider(exclude_failed)
       rewritten_req = rewriter.rewrite(provider, req.clone())
       response = send_to_provider(rewritten_req).await

       if is_success(response) {
           return response
       } else if is_quota_error(response) {
           provider_manager.mark_quota_exhausted(provider.name)
           continue
       } else if is_failover_eligible(response) {
           provider_manager.mark_failure(provider.name)
           continue
       } else {
           return response
       }
   }
   ```

4. 实现 SSE 分支：
   - 检测 SSE 请求（`stream::is_sse_request`）
   - 走 streaming 透传，而不是缓冲后再返回
   - 流中途失败时不做 retry

5. 在每次响应后更新 provider 健康状态

## 历史验证方式

运行：

```bash
cargo build
```

预期：
- 无编译错误
- 所有依赖已解析

## 依赖
- 任务 005（负责 URL / Header 改写的 Rewriter）
- 任务 007（负责 SSE 处理的 Stream 模块）
