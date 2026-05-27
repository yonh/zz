# API Converter Manual Acceptance

**Phase:** P8 - Verification  
**Date:** 2026-05-04  
**Purpose:** Manual verification of API converter functionality before release

---

## Prerequisites

1. Configure an OpenAI-compatible provider (e.g., SenseNova, DeepSeek, or any OpenAI Chat API) with `api_type="openai-chat"`
2. Configure an Anthropic provider for reverse conversion tests
3. Start ZZ proxy with the configuration

---

## Acceptance Steps

### Step 1: Configure OpenAI-Type Provider

☐ Configure provider with `api_type="openai-chat"` in `config.toml`:

```toml
[[providers]]
name = "openai-provider"
base_url = "https://api.openai.com/v1"
api_key = "sk-your-key"
api_type = "openai-chat"
models = ["gpt-4", "gpt-3.5-turbo"]
```

☐ Start ZZ proxy: `cargo run --release`

**Date:** __________  
**Verified by:** __________

---

### Step 2: A2O Non-Streaming Request

☐ Send Anthropic schema request to `/a2o/v1/messages`:

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-key" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

☐ Verify response is in Anthropic schema format  
☐ Verify response header `X-Conversion-Status: success` (if present)

**Date:** __________  
**Verified by:** __________

---

### Step 3: A2O Streaming Request

☐ Send streaming request with `stream: true`:

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-key" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "stream": true,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

☐ Verify SSE events: `message_start`, `content_block_delta`, `message_delta`, `message_stop`

**Date:** __________  
**Verified by:** __________

---

### Step 4: Unknown Fields Skip

☐ Send request with unknown fields:

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-key" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "top_k": 40,
    "anthropic_beta": ["prompt-caching-2024-01-29"],
    "unknown_field": "value",
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

☐ Verify response is successful (unknown fields are skipped)  
☐ Check logs for `field_skipped` events (if telemetry enabled)

**Date:** __________  
**Verified by:** __________

---

### Step 5: Request-Side Degradation

☐ Send invalid JSON to trigger request-side error:

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-key" \
  -d '{"invalid": json'
```

☐ Verify HTTP 502 status code  
☐ Verify response body is in Anthropic error format  
☐ Verify response header `X-Conversion-Phase: request` (if present)

**Date:** __________  
**Verified by:** __________

---

### Step 6: Response-Side Degradation

☐ Mock upstream response with missing `choices` field (requires test setup)

☐ Verify upstream response is passed through unchanged  
☐ Verify response header `X-Conversion-Status: failed` (if present)  
☐ Verify response header `X-Conversion-Phase: response` (if present)

**Date:** __________  
**Verified by:** __________  
**Note:** This step requires test infrastructure to mock upstream responses

---

### Step 7: `/v1/*` Passthrough Regression

☐ Configure Anthropic provider with `api_type="anthropic"`:

```toml
[[providers]]
name = "anthropic-provider"
base_url = "https://api.anthropic.com/v1"
api_key = "sk-ant-your-key"
api_type = "anthropic"
models = ["claude-3-opus", "claude-3-sonnet"]
```

☐ Send request to `/v1/messages` (standard path, no conversion):

```bash
curl -X POST http://127.0.0.1:9090/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-ant-your-key" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

☐ Verify behavior is identical to pre-upgrade (byte-level passthrough)

**Date:** __________  
**Verified by:** __________

---

### Step 8: O2A Reverse Conversion

☐ Configure Anthropic provider  
☐ Send OpenAI schema request to `/o2a/v1/chat/completions`:

```bash
curl -X POST http://127.0.0.1:9090/o2a/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-key" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

☐ Verify non-streaming conversion works  
☐ Verify streaming conversion works  
☐ Verify unknown fields are skipped  
☐ Verify degradation behavior

**Date:** __________  
**Verified by:** __________

---

### Step 9: Configuration Compatibility

☐ Use pre-upgrade `config.toml` (without new fields)  
☐ Start ZZ proxy: `cargo run --release -- -c config.toml`

☐ Verify no errors on startup  
☐ Verify all `/v1/*` routes work identically to pre-upgrade  
☐ Verify config loads with default values for new fields

**Date:** __________  
**Verified by:** __________

---

## Notes

- Steps 6 requires test infrastructure to mock upstream responses - marked as optional for manual acceptance
- Telemetry events (field_skipped, etc.) require enabling `observability.telemetry.enabled = true` in config
- Response headers `X-Conversion-*` are only present on conversion paths (`/a2o/*`, `/o2a/*`), not on `/v1/*` passthrough

---

## Overall Acceptance Status

☐ All 9 steps completed successfully  
☐ Date: __________  
☐ Verified by: __________
