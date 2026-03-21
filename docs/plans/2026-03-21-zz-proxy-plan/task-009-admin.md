# Task 009: Admin Endpoints - Health/Stats/Reload

## Goal
Implement admin HTTP endpoints for monitoring and configuration management.

## BDD Scenarios

```gherkin
Scenario: Health endpoint returns provider states
  Given three providers: ali-account-1 (healthy), zhipu-account-1 (cooldown), ali-account-2 (unhealthy)
  When GET /zz/health is requested
  Then response is JSON with provider states
  And shows ali-account-1 as healthy
  And shows zhipu-account-1 as cooldown
  And shows ali-account-2 as unhealthy

Scenario: Stats endpoint returns request/error counts
  Given proxy has handled 10 requests to ali-account-1
  And ali-account-1 has 2 quota errors
  When GET /zz/stats is requested
  Then response includes request_count=10 for ali-account-1
  And error_count=2 for ali-account-1

Scenario: Reload endpoint hot-reloads config
  Given config.toml is modified on disk
  When POST /zz/reload is requested
  Then config is reloaded without restarting server
  And new providers are available immediately
  And in-flight requests are not dropped

Scenario: Non-admin paths are not intercepted
  Given request path = /v1/chat/completions
  When request reaches admin router
  Then admin router returns None (not handled)
  And request continues to proxy handler

Scenario: Admin paths are intercepted
  Given request path = /zz/health
  When request reaches admin router
  Then admin router returns Some(response)
  And proxy handler is not invoked
```

## Files to Create/Edit

**Create**:
- `src/admin.rs` - Complete implementation

## Implementation Steps

1. Define admin router function:
   - `handle_admin_request(req: &Request<Body>) -> Option<Response<Body>>`
   - Match on path prefix `/zz/`
   - Return Some(response) if handled, None if not admin path

2. Implement `/zz/health` endpoint:
   - GET only
   - Return JSON: `{ "providers": [{ "name": "...", "state": "...", "failure_count": 0 }] }`
   - Include health state, cooldown time remaining, failure count

3. Implement `/zz/stats` endpoint:
   - GET only
   - Return JSON with request counts per provider
   - Include error counts, last error timestamp
   - Track metrics in Provider struct (AtomicU64 counters)

4. Implement `/zz/reload` endpoint:
   - POST only
   - Reload config from disk
   - Update ProviderManager with new config
   - Return success/failure status
   - **Important**: Use Arc swap or read-write lock for safe hot-reload

5. Integrate with main request handler:
   - Check admin path before proxying
   - If admin handles it, return response immediately

## Verification

Run server and test:
```bash
curl http://localhost:9090/zz/health
curl http://localhost:9090/zz/stats
curl -X POST http://localhost:9090/zz/reload
```

Expected:
- All endpoints return valid JSON
- Health shows correct provider states
- Reload succeeds without crashing

## Dependencies
- Task 008 (Proxy module for request handling integration)
