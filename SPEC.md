# ZZ - LLM API Reverse Proxy with Auto-Failover

## Overview

A lightweight, high-performance reverse proxy written in Rust that sits between coding tools (Claude Code, Cursor, etc.) and multiple upstream LLM API providers. It exposes a single unified endpoint and automatically rotates/fails over across multiple provider accounts when quota limits are hit.

## Problem

Coding tools like Claude Code have per-plan token limits. Users with multiple accounts across different providers (Alibaba DashScope, Zhipu GLM, OpenAI, etc.) need a way to transparently pool these accounts and auto-switch when one is exhausted.

## Architecture

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐
│ Coding Tool  │────▶│   ZZ Proxy       │────▶│ Provider A (Ali) │
│ (Claude Code)│     │                  │     └──────────────────┘
│              │◀────│  - URL rewrite   │     ┌──────────────────┐
└─────────────┘     │  - Header rewrite│────▶│ Provider B (Zhipu)│
                    │  - Failover logic│     └──────────────────┘
                    │  - Quota tracking│     ┌──────────────────┐
                    └──────────────────┘────▶│ Provider C (...)  │
                                             └──────────────────┘
```

## Core Design Principles

1. **Body-transparent**: Request and response bodies are streamed through without parsing or modification (including SSE)
2. **Header-aware**: Rewrites `Authorization` header and `Host` per upstream provider
3. **URL-rewriting**: Maps local path to upstream base URL + path
4. **Failover-driven**: Detects quota exhaustion and auto-switches to next provider
5. **Zero-downtime rotation**: Seamless switching without dropping the current request (retry on failover-eligible errors)

## Configuration

File: `config.toml`

```toml
[server]
listen = "127.0.0.1:9090"          # Local listen address
# request_timeout_secs = 300       # Per-request timeout (default: 300s for long LLM calls)
# log_level = "info"               # trace | debug | info | warn | error

[routing]
strategy = "failover"              # failover | round-robin | weighted-random
# retry_on_failure = true          # Retry current request on next provider if current fails
# max_retries = 3                  # Max retry attempts per request

# Health check: temporarily disable a provider after consecutive failures
[health]
# failure_threshold = 3            # Mark unhealthy after N consecutive failures
# recovery_secs = 600              # Re-check unhealthy provider after N seconds
# cooldown_secs = 60               # Cooldown after a quota error before retrying same provider

# --- Provider Definitions ---
# Each [[providers]] entry defines one upstream endpoint.
# Multiple entries can point to the same vendor with different API keys (multi-account).

[[providers]]
name = "ali-account-1"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"  # OpenAI-compatible endpoint
api_key = "sk-xxxx"
# priority = 1                     # Lower = higher priority (used in failover strategy)
# weight = 5                       # Relative weight (used in weighted-random strategy)
# models = ["qwen-plus", "qwen-turbo"]  # Optional: only route requests for these models to this provider
# headers = { "X-Custom" = "val" } # Optional: extra headers to inject

[[providers]]
name = "zhipu-account-1"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "sk-yyyy"
# priority = 2
# models = ["glm-4", "glm-4-flash"]

[[providers]]
name = "ali-account-2"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-zzzz"
# priority = 3
```

## Routing Strategies

### 1. Failover (Default)
- Try providers in priority order (lowest `priority` value first)
- On quota/rate-limit error → mark provider as cooldown → try next
- On other errors (5xx, timeout) → retry on next provider
- Healthy providers are always preferred over cooldown ones

### 2. Round-Robin
- Distribute requests evenly across all healthy providers
- Skip unhealthy/cooldown providers

### 3. Weighted-Random
- Random selection weighted by `weight` field
- Skip unhealthy/cooldown providers

## Failover Detection

The proxy identifies quota exhaustion by:

| Signal | Action |
|--------|--------|
| HTTP 429 (Too Many Requests) | Cooldown provider, retry next |
| HTTP 403 with quota-related body keywords | Cooldown provider, retry next |
| HTTP 5xx | Mark failure, retry next |
| Connection timeout / refused | Mark failure, retry next |
| HTTP 2xx | Reset failure counter, pass through |

**Quota keywords** (checked case-insensitively in first 1KB of error response body):
- `quota`, `rate limit`, `exceeded`, `insufficient_quota`, `billing`, `limit reached`

> Note: Body inspection is only done on **error responses** (non-2xx), never on success responses. This keeps the proxy transparent for normal traffic.

## Request Flow

```
1. Client sends request to proxy (e.g., POST /v1/chat/completions)
2. Proxy selects provider based on routing strategy
3. Proxy rewrites:
   - URL: {provider.base_url} + request path
   - Header: Authorization → Bearer {provider.api_key}
   - Header: Host → {provider.host}
4. Proxy forwards request body as-is (streaming)
5. Proxy reads response:
   - If 2xx → stream response back to client as-is
   - If failover-eligible error → retry with next provider (up to max_retries)
   - If all providers exhausted → return last error to client
6. Update provider health state
```

## Streaming (SSE) Support

Critical for LLM APIs. The proxy must:

- Detect `Accept: text/event-stream` or `stream: true` in request
- Use chunked transfer encoding for upstream and downstream
- Pipe upstream SSE chunks to client in real-time with zero buffering
- **No retry on mid-stream failure** (only pre-response failover)

## API Endpoints

The proxy exposes:

| Endpoint | Description |
|----------|-------------|
| `/*` (all paths) | Transparent proxy to upstream |
| `GET /zz/health` | Proxy health check (JSON: provider states) |
| `GET /zz/stats` | Request/error counts per provider |
| `POST /zz/reload` | Hot-reload config without restart |

Admin endpoints are prefixed with `/zz/` to avoid collision with upstream API paths.

## Module Structure

```
src/
├── main.rs              # Entry point, CLI args, server startup
├── config.rs            # Config parsing (TOML) and validation
├── proxy.rs             # Core proxy handler (request/response forwarding)
├── router.rs            # Provider selection logic (failover/round-robin/weighted)
├── provider.rs          # Provider state management (health, cooldown, counters)
├── rewriter.rs          # URL and header rewriting
├── health.rs            # Health check and recovery logic
├── stream.rs            # SSE/chunked streaming utilities
├── admin.rs             # /zz/* admin endpoints
├── error.rs             # Error types
└── logging.rs           # Structured logging setup
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `hyper` + `hyper-util` | HTTP server and client |
| `http-body-util` | Body streaming utilities |
| `toml` + `serde` | Config parsing |
| `tracing` + `tracing-subscriber` | Structured logging |
| `dashmap` or `arc-swap` | Lock-free shared state for provider health |
| `clap` | CLI argument parsing |

## Non-Goals (V1)

- **No request body parsing**: We don't inspect or modify request/response content
- **No authentication on proxy**: Proxy is local-only (127.0.0.1), no auth needed
- **No TLS termination**: Upstream uses HTTPS via `hyper-tls` or `rustls`, but proxy listens on plain HTTP
- **No caching**: Every request is forwarded
- **No model mapping**: Provider must support the model name the client sends as-is
- **No token counting**: Quota detection is based on HTTP errors, not token usage tracking

## Future Considerations (V2+)

- **Token budget tracking**: Parse usage from response to proactively switch before hitting limit
- **Model aliasing**: Map model names between providers (e.g., `gpt-4` → `qwen-plus`)
- **Web dashboard**: Real-time stats and provider management UI
- **Config encryption**: Encrypt API keys at rest
- **Multi-protocol**: Native Anthropic API support (non-OpenAI format)
- **Request logging**: Optional request/response logging for debugging

## Success Criteria

1. `curl` through proxy to Ali/Zhipu returns correct LLM response
2. SSE streaming works with zero perceivable latency overhead
3. When provider A returns 429, next request automatically goes to provider B
4. Provider A auto-recovers after cooldown period
5. Hot-reload config without dropping in-flight requests
6. < 1ms proxy overhead per request (excluding network)
