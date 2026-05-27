# Manual Verification Commands

This document provides curl commands to verify the Claude Code → OpenAI Chat conversion implementation.

## Prerequisites
- Start the zz server: `cargo run`
- Have a provider configured with `api_type: openai-chat` or `api_type: auto`

## Test 1: Non-streaming conversion (POST /a2o/v1/messages)

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: test-key" \
  -d '{
    "model": "claude-3-opus-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

Expected: Returns Anthropic-formatted response, but upstream calls OpenAI Chat completions.

## Test 2: Streaming conversion (POST /a2o/v1/messages with stream:true)

```bash
curl -X POST http://127.0.0.1:9090/a2o/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: test-key" \
  -d '{
    "model": "claude-3-opus-20240229",
    "max_tokens": 1024,
    "stream": true,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

Expected: Returns Anthropic SSE events (message_start, content_block_delta, message_stop), upstream calls OpenAI Chat completions.

## Test 3: Compat mode (POST /v1/messages with anthropic-version header)

```bash
curl -X POST http://127.0.0.1:9090/v1/messages \
  -H "Content-Type: application/json" \
  -H "anthropic-version: 2023-06-01" \
  -H "x-api-key: test-key" \
  -d '{
    "model": "claude-3-opus-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

Expected: Same as Test 1 - Anthropic request converted to OpenAI Chat upstream.

## Test 4: Transparent proxy (POST /v1/chat/completions - no conversion)

```bash
curl -X POST http://127.0.0.1:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "x-api-key: test-key" \
  -d '{
    "model": "gpt-4",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

Expected: Passes through unchanged to upstream (byte-level transparency).

## Verification Checklist

- [ ] Non-streaming conversion returns correct format
- [ ] Streaming conversion returns SSE events
- [ ] Compat mode triggers with anthropic-version header
- [ ] Transparent proxy remains unchanged
- [ ] Response headers include X-Conversion-Status
