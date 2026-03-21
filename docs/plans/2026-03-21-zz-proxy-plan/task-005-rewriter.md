# Task 005: Rewriter Module - URL and Header Rewriting

## Goal
Implement URL and header rewriting to map local requests to upstream provider endpoints.

## BDD Scenarios

```gherkin
Scenario: Rewrite URL with base_url + request path
  Given provider.base_url = "https://dashscope.aliyuncs.com/compatible-mode"
  And request path = "/v1/chat/completions"
  When rewrite_url() is called
  Then returns "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"

Scenario: Rewrite Authorization header
  Given provider.api_key = "sk-xxxx"
  When rewrite_headers() is called
  Then Authorization header equals "Bearer sk-xxxx"

Scenario: Rewrite Host header from base_url
  Given provider.base_url = "https://dashscope.aliyuncs.com/compatible-mode"
  When rewrite_headers() is called
  Then Host header equals "dashscope.aliyuncs.com"

Scenario: Preserve existing headers except rewritten ones
  Given request has headers: User-Agent, Content-Type, Accept
  When rewrite_headers() is called
  Then all three headers are preserved
  And Authorization and Host are added/replaced

Scenario: Inject custom provider headers
  Given provider.headers = { "X-Custom" = "value" }
  When rewrite_headers() is called
  Then X-Custom header equals "value"
```

## Files to Create/Edit

**Create**:
- `src/rewriter.rs` - Complete implementation

## Implementation Steps

1. Implement URL rewriting:
   - Parse base_url to extract host/port
   - Join base_url path with request path
   - Handle trailing slash edge cases
   - Use `url::Url` crate for proper URL manipulation

2. Implement header rewriting:
   - Replace Authorization: `Bearer {api_key}`
   - Replace Host: extracted from base_url
   - Merge custom headers from provider config
   - Preserve all other headers from original request

3. Create RequestRewriter struct:
   - `rewrite_request(&self, provider: &Provider, req: Request<Body>) -> Request<Body>`
   - Returns modified request with rewritten URL and headers

4. Handle edge cases:
   - Empty path (should use base_url as-is)
   - Query parameters (preserve from original request)
   - Fragment (preserve from original request)

## Verification

Run:
```bash
cargo test --lib rewriter
```

Expected:
- URL joining tests pass
- Header replacement tests pass
- Custom header injection tests pass

## Dependencies
- Task 004 (Router provides Provider struct)
