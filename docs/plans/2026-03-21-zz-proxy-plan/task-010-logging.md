# Task 010: Logging Module - Structured Logging

## Goal
Implement structured logging with configurable log levels using tracing.

## BDD Scenarios

```gherkin
Scenario: Log request start with method and path
  Given client sends POST /v1/chat/completions
  When request starts processing
  Then log entry contains: level=INFO, method=POST, path=/v1/chat/completions

Scenario: Log provider selection
  Given router selects ali-account-1
  When proxy forwards request
  Then log entry contains: level=DEBUG, provider=ali-account-1, action=selected

Scenario: Log quota error with provider name
  Given ali-account-1 returns 429
  When error is detected
  Then log entry contains: level=WARN, provider=ali-account-1, error=quota_exhausted

Scenario: Log successful response with status
  Given upstream returns 200 OK
  When response is sent to client
  Then log entry contains: level=INFO, status=200, duration_ms=123

Scenario: Respect configured log level
  Given config.log_level = "warn"
  When debug message is logged
  Then debug message is not output
  And warn/error messages are output

Scenario: Structured JSON output
  Given log output format = JSON
  When any log entry is written
  Then output is valid JSON with fields: timestamp, level, message, and context fields
```

## Files to Create/Edit

**Create**:
- `src/logging.rs` - Complete implementation

## Implementation Steps

1. Initialize tracing subscriber:
   - Use `tracing_subscriber::fmt()` with JSON formatter
   - Configure log level from config (or env var)
   - Add timestamp, target, and span support

2. Define structured log macros/events:
   - Use `tracing::info!`, `tracing::debug!`, etc. with structured fields
   - Example: `tracing::info!(method = %req.method(), path = %req.uri().path(), "request started")`

3. Add log points throughout code:
   - Request start/end (with duration)
   - Provider selection
   - Provider failure/health changes
   - Quota detection
   - Admin endpoint access
   - Config reload

4. Implement log configuration:
   - Read `log_level` from config
   - Support environment override (e.g., `RUST_LOG` env var)
   - Default to "info" if not specified

5. Add request ID for correlation (optional):
   - Generate unique ID per request
   - Include in all logs for that request
   - Helps trace request flow across components

## Verification

Run:
```bash
cargo build
RUST_LOG=debug cargo run 2>&1 | head -20
```

Expected:
- Logs show structured output
- Log level filtering works correctly
- All key events are logged

## Dependencies
- Task 001 (tracing dependency in Cargo.toml)
