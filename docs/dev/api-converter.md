# API Converter Developer Guide

**Phase:** P8 - Verification  
**Last Updated:** 2026-05-04

---

## Overview

The API Converter module enables protocol conversion between different LLM API formats. It allows clients using one API format (e.g., Anthropic) to communicate with providers using another format (e.g., OpenAI Chat) transparently.

---

## Architecture

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │ HTTP Request
       ▼
┌─────────────────────────────────┐
│   main.rs - Route Dispatcher    │
│   - Prefix matching            │
│   - Inject (source, target)    │
└──────┬──────────────────────────┘
       │
       ▼
┌─────────────────────────────────┐
│  proxy::conversion_proxy_handler│
│  - Read request body             │
│  - convert_request()             │
│  - Forward to upstream          │
│  - convert_response()           │
│  - Write response              │
└──────┬──────────────────────────┘
       │
       ├──────────────┬──────────────┐
       ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  converter   │ │  converter   │ │  converter   │
│  (A→O)       │ │  (O→A)       │ │  (stream)    │
└──────────────┘ └──────────────┘ └──────────────┘
       │              │              │
       └──────────────┴──────────────┘
                      │
                      ▼
           ┌──────────────────┐
           │  Provider Layer  │
           │  - API type     │
           │  - Selection     │
           └──────────────────┘
```

---

## Route Prefix Semantics

See [route-matrix.md](../plans/2026-05-04-api-converter-plan/route-matrix.md) for the complete route matrix.

| Prefix | Source → Target | Inbound Path | Upstream Path | Status |
|--------|----------------|--------------|---------------|--------|
| `/v1/*` | None (passthrough) | `/v1/*` | `/v1/*` | Existing |
| `/a2o/v1/*` | Anthropic → OpenAIChat | `/a2o/v1/messages` | `/v1/chat/completions` | P4 (Implemented) |
| `/o2a/v1/*` | OpenAIChat → Anthropic | `/o2a/v1/chat/completions` | `/v1/messages` | P4 (Implemented) |
| `/a2r/v1/*` | Anthropic → OpenAIResponses | `/a2r/v1/messages` | `/v1/responses` | Future |
| `/r2a/v1/*` | OpenAIResponses → Anthropic | `/r2a/v1/responses` | `/v1/messages` | Future |

**Prefix Naming Rule:** `<source-code><2><target-code>` (e.g., `a2o` = Anthropic to OpenAI)

---

## Claude Code Compatibility Mode

The compatibility mode allows Claude Code (and other Anthropic-style clients) to use OpenAI Chat providers without modifying their endpoint paths. When enabled, requests to `/v1/messages` with Anthropic-style headers are automatically converted to the target API format.

### Configuration

Add the following to your `config.toml`:

```toml
[compat.claude_code_openai]
enabled = true
match_paths = ["/v1/messages"]
target_api_type = "openai-chat"
```

### How It Works

1. **Request Detection**: The proxy checks if:
   - Compatibility mode is enabled
   - The request path matches `match_paths` (default: `/v1/messages`)
   - The request contains an `anthropic-version` header (indicates Anthropic-style client)

2. **Conversion Flow**:
   ```
   Claude Code → /v1/messages (Anthropic format)
                    ↓
            Request: Anthropic → OpenAI Chat
                    ↓
            Upstream: OpenAI Chat provider
                    ↓
            Response: OpenAI Chat → Anthropic
                    ↓
            Claude Code receives Anthropic format
   ```

3. **Fallback**: If compatibility mode is disabled or the request doesn't match the Claude Code pattern, the request falls back to transparent proxy behavior (passes through to `/v1/*` unchanged).

### Usage with Claude Code

To use Claude Code with an OpenAI Chat provider:

1. Configure the proxy with an OpenAI Chat provider:
   ```toml
   [[providers]]
   name = "openai-provider"
   base_url = "https://api.openai.com/v1"
   api_key = "sk-your-key"
   api_type = "openai-chat"
   models = ["gpt-4", "gpt-3.5-turbo"]
   ```

2. Enable compatibility mode in config:
   ```toml
   [compat.claude_code_openai]
   enabled = true
   ```

3. Configure Claude Code to use the proxy as the base URL:
   ```
   ANTHROPIC_BASE_URL=http://localhost:9090
   ```

Claude Code will now send requests to `/v1/messages` as usual, and the proxy will automatically convert them to OpenAI Chat format.

### Comparison with Explicit Prefix Mode

| Mode | Endpoint Path | Configuration Required | Use Case |
|------|--------------|----------------------|----------|
| Explicit Prefix (`/a2o/v1/messages`) | Client uses `/a2o/v1/messages` | None (always enabled) | Explicit conversion, clear intent |
| Compatibility Mode (`/v1/messages`) | Client uses `/v1/messages` (standard) | `compat.claude_code_openai.enabled = true` | Zero-config for Claude Code, drop-in replacement |

### Known Limitations

1. **Streaming**: SSE streaming is implemented but collects the full stream before conversion (not true streaming). This works for most use cases but may have higher latency.
2. **Header Detection**: Currently only checks for `anthropic-version` header. Future versions may add more sophisticated detection (e.g., request body schema validation).
3. **Path Matching**: Only exact path matches are supported (no wildcard patterns in `match_paths`).

---

## Field Mapping Summary

See [field-mapping.md](../plans/2026-05-04-api-converter-plan/field-mapping.md) for detailed field mappings.

### Anthropic → OpenAI Chat (A2O)

**Request:**
- `model` → `model`
- `max_tokens` → `max_tokens` (or `max_completion_tokens` for reasoning models)
- `messages[]` → `messages[]` (with content block transformation)
- `system` → `system` (as first message with role=system)
- `tools[]` → `tools[]` (format transformation)
- `tool_choice` → `tool_choice` (format transformation)
- `temperature`, `top_p`, `top_k` → mapped directly

**Response:**
- `id` → `id`
- `type` → N/A (OpenAI uses object field)
- `content[]` → `choices[].message.content`
- `stop_reason` → `finish_reason`
- `usage` → `usage`

### OpenAI Chat → Anthropic (O2A)

**Request:**
- `model` → `model`
- `messages[]` → `messages[]` (with content transformation)
- `tools[]` → `tools[]` (format transformation)
- `tool_choice` → `tool_choice` (format transformation)

**Response:**
- `id` → `id`
- `choices[].message.content` → `content[]`
- `finish_reason` → `stop_reason`
- `usage` → `usage`

---

## Error Model and Degradation

See [error-model.md](../plans/2026-05-04-api-converter-plan/error-model.md) for the complete error model.

### Error Handling Strategy

The converter implements graceful degradation in three phases:

1. **Request-side errors:** Return 502 with source format error body
2. **Response-side errors:** Pass through upstream response with failure headers (if `enable_conversion_fallback=true`)
3. **Streaming errors:** Gracefully close stream with `message_stop` event

### Error Types

| Error Code | Kind | Description |
|------------|------|-------------|
| `invalid_json` | InvalidJson | Request/response body is not valid JSON |
| `schema_mismatch` | SchemaMismatch | Required field missing or wrong type |
| `unsupported_feature` | UnsupportedFeature | Requested feature not implemented |
| `sse_parse` | StreamProtocol | SSE parsing error in streaming response |
| `internal` | Internal | Unexpected internal error |
| `not_implemented` | NotImplemented | Converter not yet implemented |

### Response Headers

Conversion responses include diagnostic headers:

- `X-Conversion-Status`: `success` | `failed`
- `X-Conversion-Phase`: `request` | `response` | `stream`
- `X-Conversion-Error`: Error code (if failed)

---

## Iteration Loop Workflow (from P9)

The telemetry system enables active discovery and iterative fixing of field mappings:

```
┌─────────────────┐
│  Production Use │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Telemetry      │
│  - Events       │
│  - Samples      │
│  - Coverage     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Admin API     │
│  /zz/api/...   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Replay Tool    │
│  convert-replay │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Fix Mapping    │
│  Update Code    │
└────────┬────────┘
         │
         └──────┘
```

### Telemetry Endpoints

- `GET /zz/api/conversion/events` - Query conversion events
- `GET /zz/api/conversion/samples` - Get samples by signature
- `GET /zz/api/conversion/samples/{id}/body` - Get sample body
- `GET /zz/api/conversion/coverage` - Get field coverage metrics
- `POST /zz/api/conversion/samples/clear` - Clear samples

### Replay Tool

```bash
cargo run --bin convert-replay -- \
  --file sample.json \
  --direction request \
  --source Anthropic \
  --target OpenAIChat
```

---

## Extension Guide

To add a new route prefix for a new API conversion:

### Step 1: Add ApiType Enum Variant

In `src/converter.rs`:

```rust
pub enum ApiType {
    Anthropic,
    OpenAIChat,
    OpenAICompletions,
    OpenAIResponses,
    Gemini,  // Add new type
    Unknown,
}
```

### Step 2: Update Route Matrix

In `docs/plans/2026-05-04-api-converter-plan/route-matrix.md`:

Add row to §1 table:
| `/g2a/v1/*` | Gemini → Anthropic | Gemini → Anthropic | Future |

Add row to §2 table:
| `/g2a/v1/generateContent` | Gemini | Anthropic | `/v1/messages` | Future |

### Step 3: Implement Converter

Create `src/converter/gemini_to_anthropic.rs`:

```rust
pub struct GeminiToAnthropicConverter;

impl ApiConverter for GeminiToAnthropicConverter {
    fn convert_request(&self, body: &Bytes, target: ApiType) -> Result<Bytes, ConversionError> {
        // Implementation
    }

    fn convert_response(&self, body: &Bytes, source: ApiType, target: ApiType, is_stream: bool) -> Result<Bytes, ConversionError> {
        // Implementation
    }
}
```

### Step 4: Add target_path() Branch

In `src/converter.rs` `target_path()` function:

```rust
pub fn target_path(source: ApiType, target: ApiType, inbound_path: &str) -> Result<String, ConversionError> {
    match (source, target, inbound_path) {
        // Existing cases...
        (ApiType::Gemini, ApiType::Anthropic, "/g2a/v1/generateContent") => Ok("/v1/messages".to_string()),
        _ => Err(ConversionError::new(
            ConversionErrorKind::UnsupportedFeature,
            "unsupported_path",
            format!("Unsupported path: {}", inbound_path),
        )),
    }
}
```

### Step 5: Add Route Handler

In `src/main.rs` route dispatcher:

```rust
// Add before /v1/ match
if path.starts_with("/g2a/v1/") {
    return conversion_proxy_handler(req, state, ApiType::Gemini, ApiType::Anthropic).await;
}
```

### Step 6: Update Provider Selection

In `docs/plans/2026-05-04-api-converter-plan/route-matrix.md` §5:

```
target=Gemini ⇒ only match api_type ∈ {"gemini","auto"} providers
```

### Step 7: Add Tests

Create `tests/integration_conversion_g2a.rs` with unit and integration tests.

Add manual acceptance step to `docs/active-work/api-converter/manual-acceptance.md`.

---

## Configuration

### Provider Configuration

```toml
[[providers]]
name = "openai-provider"
base_url = "https://api.openai.com/v1"
api_key = "sk-your-key"
api_type = "openai-chat"  # or "anthropic", "auto"
enable_conversion_fallback = true  # Pass through on response-side errors
models = ["gpt-4", "gpt-3.5-turbo"]
```

### Telemetry Configuration

```toml
[observability.telemetry]
enabled = true
sample_max_count = 10000
sample_max_bytes = 67108864  # 64 MiB
sample_resave_every = 100
persist_path = ""  # Empty = in-memory only
unknown_field_log_level = "warn"
redact_extra_headers = []
```

---

## Testing

### Unit Tests

```bash
cargo test converter
```

### Integration Tests

```bash
cargo test integration_conversion
```

### Manual Acceptance

See [docs/active-work/api-converter/manual-acceptance.md](../active-work/api-converter/manual-acceptance.md)

---

## Known Limitations

1. Streaming conversion is partially implemented (P5)
2. Some advanced features (e.g., tool result caching) not yet mapped
3. Telemetry infrastructure is in place but not yet wired into converters (instrumentation is a future task)
4. Integration tests with mock HTTP servers are placeholders

---

## References

- [route-matrix.md](../plans/2026-05-04-api-converter-plan/route-matrix.md) - Route prefix semantics and matrix
- [field-mapping.md](../plans/2026-05-04-api-converter-plan/field-mapping.md) - Detailed field mappings
- [error-model.md](../plans/2026-05-04-api-converter-plan/error-model.md) - Error handling and degradation
- [phase-P9-iteration-telemetry.md](../plans/2026-05-04-api-converter-plan/phase-P9-iteration-telemetry.md) - Telemetry system design
