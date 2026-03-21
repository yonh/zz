# Task 006: Error Module - Quota Detection and Error Types

## Goal
Implement error types and quota exhaustion detection from HTTP responses.

## BDD Scenarios

```gherkin
Scenario: Detect quota exhaustion from HTTP 429
  Given HTTP response status = 429
  When is_quota_error() is called
  Then returns true

Scenario: Detect quota exhaustion from HTTP 403 with quota keywords
  Given HTTP response status = 403
  And response body contains "quota exceeded"
  When is_quota_error() is called
  Then returns true

Scenario: Detect quota from body keywords case-insensitively
  Given HTTP response status = 403
  And response body contains "INSUFFICIENT_QUOTA"
  When is_quota_error() is called
  Then returns true (case-insensitive match)

Scenario: Don't detect quota from HTTP 200
  Given HTTP response status = 200
  When is_quota_error() is called
  Then returns false (never inspect body on success)

Scenario: Detect other failover-eligible errors
  Given HTTP response status = 500
  When is_failover_eligible() is called
  Then returns true (retry on next provider)

Scenario: Non-failover errors
  Given HTTP response status = 400
  When is_failover_eligible() is called
  Then returns false (client error, don't retry)
```

## Files to Create/Edit

**Create**:
- `src/error.rs` - Complete implementation

## Implementation Steps

1. Define custom error types:
   ```rust
   enum ProxyError {
       ConfigError(String),
       ProviderError(String),
       RequestError(String),
       QuotaExhausted(String),
       AllProvidersFailed(Vec<ProxyError>),
   }
   ```

2. Implement quota detection:
   - `is_quota_error(status, body: &[u8]) -> bool`
   - Check status codes: 429 → true, 403 → check body
   - Check body for keywords: "quota", "rate limit", "exceeded", "insufficient_quota", "billing", "limit reached"
   - Only inspect first 1KB of body
   - Case-insensitive matching

3. Implement failover eligibility:
   - `is_failover_eligible(status) -> bool`
   - True for: 429, 403(quota), 5xx, timeout, connection error
   - False for: 2xx, 400, 401, 404

4. Implement error conversion from hyper/http errors

## Verification

Run:
```bash
cargo test --lib error
```

Expected:
- Quota detection tests pass for all keyword variations
- Status code classification tests pass
- Case-insensitive matching tests pass

## Dependencies
- Task 002 (Config types for error contexts)
