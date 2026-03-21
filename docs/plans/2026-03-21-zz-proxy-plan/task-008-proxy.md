# Task 008: Proxy Module - Request/Response Forwarding

## Goal
Implement core proxy handler that forwards requests to upstream providers with failover and retry logic.

## BDD Scenarios

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

## Files to Create/Edit

**Create**:
- `src/proxy.rs` - Complete implementation

## Implementation Steps

1. Create HTTP client (hyper client with TLS):
   - Use `hyper_util::client::legacy::Client` with HTTPS connector
   - Configure timeouts from config

2. Implement proxy handler:
   - `proxy_handler(req: Request<Incoming>, state: AppState) -> Result<Response<Full<Bytes>>, Error>`
   - Extract request path and headers
   - Call router.select_provider()
   - If None → return 503 (no available providers)

3. Implement request forwarding loop:
   ```rust
   for attempt in 0..max_retries {
       provider = router.select_provider(exclude_failed)
       rewritten_req = rewriter.rewrite(provider, req.clone())
       response = send_to_provider(rewritten_req).await

       if is_success(response) {
           return response
       } else if is_quota_error(response) {
           provider_manager.mark_quota_exhausted(provider.name)
           continue // retry
       } else if is_failover_eligible(response) {
           provider_manager.mark_failure(provider.name)
           continue // retry
       } else {
           return response // don't retry
       }
   }
   ```

4. Implement SSE path:
   - Detect SSE request (stream::is_sse_request)
   - Use streaming instead of buffering
   - No retry on mid-stream failure

5. Update provider health on each response

## Verification

Run:
```bash
cargo build
```

Expected: Compiles without errors, all dependencies resolved.

## Dependencies
- Task 005 (Rewriter for URL/header modification)
- Task 007 (Stream module for SSE handling)
