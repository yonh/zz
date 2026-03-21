# Task 012: Integration Test - Manual Verification

## Goal
Verify all core features work end-to-end with manual testing.

## BDD Scenarios

```gherkin
Scenario: Proxy forwards request to upstream provider
  Given zz server is running with config pointing to Ali provider
  And Ali provider is healthy
  When curl sends request to http://localhost:9090/v1/chat/completions
  Then receives valid LLM response from Ali
  And response status is 200

Scenario: SSE streaming works with zero latency
  Given zz server is running
  When curl sends request with stream=true
  Then receives SSE events in real-time
  And no buffering delay is observed

Scenario: Quota error triggers failover
  Given zz has two providers: ali-account-1, zhipu-account-1
  And ali-account-1 is configured with invalid API key (will return 403)
  When first request is sent
  Then ali-account-1 returns 403
  And zz marks it as cooldown
  And retries with zhipu-account-1
  And client receives response from zhipu-account-1

Scenario: Health endpoint shows provider states
  Given zz server is running
  When GET /zz/health is requested
  Then returns JSON with all provider states
  And shows which providers are healthy/cooldown/unhealthy

Scenario: Config hot-reload works
  Given zz server is running
  And config.toml is modified (add new provider)
  When POST /zz/reload is sent
  Then new provider is available immediately
  And subsequent requests can be routed to it

Scenario: All providers exhausted returns error
  Given all providers have invalid API keys
  When request is sent
  Then zz tries all providers
  And returns last error to client (403 or 401)
```

## Test Setup

1. Create test config:
   ```bash
   cp config.toml.example config.toml
   # Edit with real API keys for testing
   ```

2. Start server:
   ```bash
   cargo run
   ```

## Manual Test Commands

```bash
# Test 1: Basic proxy
curl http://localhost:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-plus",
    "messages": [{"role": "user", "content": "hi"}]
  }'

# Test 2: SSE streaming
curl -N http://localhost:9090/v1/chat/completions \
  -H "Accept: text/event-stream" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-plus",
    "messages": [{"role": "user", "content": "tell me a story"}],
    "stream": true
  }'

# Test 3: Health endpoint
curl http://localhost:9090/zz/health | jq .

# Test 4: Stats endpoint
curl http://localhost:9090/zz/stats | jq .

# Test 5: Hot reload
curl -X POST http://localhost:9090/zz/reload

# Test 6: Trigger failover (configure first provider with bad key)
# Then watch logs to see it switch to second provider
```

## Verification Criteria

All must pass:
- ✅ Basic request forwarding works
- ✅ SSE streaming has no perceptible latency overhead
- ✅ Health endpoint returns valid JSON
- ✅ Quota errors trigger provider cooldown
- ✅ Config reload works without restart
- ✅ Proxy overhead < 1ms (measure with local providers)

## Dependencies
- Task 011 (Main server runs and accepts connections)
