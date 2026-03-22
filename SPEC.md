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
strategy = "failover"              # failover | round-robin | weighted-random | quota-aware | manual
# retry_on_failure = true          # Retry current request on next provider if current fails
# max_retries = 3                  # Max retry attempts per request
# pinned_provider = ""             # (manual strategy only) Provider name to pin all traffic to

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
# token_budget = 1000000           # Optional: monthly token budget (used in quota-aware strategy)
# headers = { "X-Custom" = "val" } # Optional: extra headers to inject per provider

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

### 4. Quota-Aware
- Track token usage per provider (parsed from response `usage` field)
- Switch to next provider when usage exceeds configured threshold (e.g., 90% of budget)
- Requires per-provider `token_budget` configuration

### 5. Manual / Fixed
- Pin all traffic to one specific provider (configured via `pinned_provider` field)
- No automatic failover; returns error if pinned provider is unavailable

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

The proxy exposes admin endpoints prefixed with `/zz/` to avoid collision with upstream API paths.

### Legacy Endpoints (CLI/curl)

| Endpoint | Description |
|----------|-------------|
| `/*` (all paths) | Transparent proxy to upstream |
| `GET /zz/health` | Proxy health check (JSON: provider states) |
| `GET /zz/stats` | Request/error counts per provider |
| `POST /zz/reload` | Hot-reload config without restart |

### Admin REST API (Web Dashboard)

| Endpoint | Description |
|----------|-------------|
| `GET /zz/api/providers` | List all providers with config, state, and stats |
| `POST /zz/api/providers` | Add a new provider at runtime |
| `GET /zz/api/providers/{name}` | Get single provider details |
| `PUT /zz/api/providers/{name}` | Update provider config |
| `DELETE /zz/api/providers/{name}` | Remove provider |
| `POST /zz/api/providers/{name}/test` | Test provider connectivity |
| `POST /zz/api/providers/{name}/enable` | Enable provider |
| `POST /zz/api/providers/{name}/disable` | Disable provider |
| `POST /zz/api/providers/{name}/reset` | Reset health/cooldown state |
| `GET /zz/api/routing` | Get routing config + model rules |
| `PUT /zz/api/routing` | Update routing strategy/parameters |
| `GET /zz/api/routing/rules` | Get model routing rules |
| `PUT /zz/api/routing/rules` | Replace model routing rules |
| `GET /zz/api/stats` | Aggregated system stats (JSON) |
| `GET /zz/api/stats/timeseries` | Time-series data for charts |
| `GET /zz/api/logs` | Paginated structured request logs |
| `GET /zz/api/config` | Get config TOML content + metadata |
| `PUT /zz/api/config` | Validate + save + hot-reload config |
| `POST /zz/api/config/validate` | Validate config without saving |
| `GET /zz/api/health` | Proxy health check |
| `GET /zz/api/version` | Version info |

### WebSocket

| Endpoint | Description |
|----------|-------------|
| `WS /zz/ws` | Real-time push: logs, provider state changes, stats snapshots (every 5s) |

### Static Files (Production)

| Endpoint | Description |
|----------|-------------|
| `GET /zz/ui/*` | Embedded web dashboard static files |

> See `SPEC-API.md` for full request/response schemas and WebSocket protocol details.

## Module Structure

```
src/
├── main.rs              # Entry point, CLI args, server startup, background tasks
├── config.rs            # Config parsing (TOML) and validation
├── proxy.rs             # Core proxy handler (request/response forwarding, log collection)
├── router.rs            # Provider selection logic (failover/round-robin/weighted/quota-aware/manual)
├── provider.rs          # Provider state management (health, cooldown, counters, latency tracking)
├── rewriter.rs          # URL and header rewriting
├── stream.rs            # SSE/chunked streaming utilities
├── admin.rs             # Legacy /zz/* admin endpoints (health, stats, reload)
├── admin_api.rs         # /zz/api/* REST endpoints for web dashboard
├── ws.rs                # WebSocket handler + broadcast channel
├── cors.rs              # CORS middleware for /zz/* endpoints
├── stats.rs             # RPM counter, time-series aggregation
├── error.rs             # Error types
└── logging.rs           # Structured logging setup + RequestLogBuffer
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

- **No request body modification**: Request/response bodies are streamed through without modification (model field is read-only parsed for logging)
- **No authentication on proxy**: Proxy is local-only (127.0.0.1), no auth needed
- **No TLS termination**: Upstream uses HTTPS via `rustls`, but proxy listens on plain HTTP
- **No caching**: Every request is forwarded
- **No model mapping**: Provider must support the model name the client sends as-is

## Future Considerations (V2+)

- **Token budget tracking**: Parse `usage` from response to proactively switch before hitting limit (quota-aware strategy)
- **Model aliasing**: Map model names between providers (e.g., `gpt-4` → `qwen-plus`)
- **Config encryption**: Encrypt API keys at rest
- **Multi-protocol**: Native Anthropic API support (non-OpenAI format)
- **Request/response body logging**: Optional full body capture for debugging

## Success Criteria

1. `curl` through proxy to Ali/Zhipu returns correct LLM response
2. SSE streaming works with zero perceivable latency overhead
3. When provider A returns 429, next request automatically goes to provider B
4. Provider A auto-recovers after cooldown period
5. Hot-reload config without dropping in-flight requests
6. < 1ms proxy overhead per request (excluding network)
