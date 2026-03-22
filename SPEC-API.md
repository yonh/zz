# ZZ - Admin API & WebSocket Specification

This document defines the complete API contract between the ZZ Rust backend and the Web Dashboard frontend. All endpoints are prefixed with `/zz/` to avoid collision with upstream LLM API paths.

---

## 1. Design Principles

1. **Single port**: Admin API, WebSocket, UI static files, and proxy all share `listen` port (default `9090`)
2. **JSON everywhere**: All API request/response bodies use `Content-Type: application/json`
3. **CORS required**: Dev mode (Vite `localhost:5173` → backend `localhost:9090`) needs CORS headers
4. **Prefix routing**: `/zz/api/*` → REST API, `/zz/ws` → WebSocket, `/zz/ui/*` → static files, everything else → proxy
5. **Stateless REST**: Each API call is self-contained; real-time push uses WebSocket only

---

## 2. CORS Configuration

All `/zz/` responses MUST include:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
Access-Control-Max-Age: 86400
```

All `OPTIONS` preflight requests to `/zz/*` MUST return `204 No Content` with the above headers.

> Production: Replace `*` with specific origin if needed.

---

## 3. REST API Endpoints

### 3.1 Providers

#### `GET /zz/api/providers`

List all providers with config, runtime state, and stats.

**Response** `200 OK`:
```jsonc
{
  "providers": [
    {
      "name": "ali-account-1",
      "base_url": "https://dashscope.aliyuncs.com/compatible-mode",
      "api_key": "sk-xxxx",          // Full key (frontend masks in UI)
      "priority": 1,
      "weight": 50,
      "enabled": true,
      "models": ["qwen-plus", "qwen-turbo"],
      "headers": { "X-Custom": "val" },
      "token_budget": null,
      "status": "healthy",           // healthy | cooldown | unhealthy | disabled
      "cooldown_until": null,        // ISO 8601 or null
      "consecutive_failures": 0,
      "stats": {
        "total_requests": 5432,
        "total_errors": 12,
        "error_rate": 0.22,          // Percentage (0-100)
        "avg_latency_ms": 1200,
        "latency_history": [1100, 1250, 1180, 1300, 1200, 1150, 1220, 1190, 1280, 1210, 1230, 1200]
      }
    }
    // ... more providers
  ]
}
```

**Implementation notes (backend)**:
- `enabled` field: Backend needs a runtime `enabled` flag per provider (separate from health state)
- `status`: Map `ProviderState::Healthy → "healthy"`, `Cooldown → "cooldown"`, `Unhealthy → "unhealthy"`. If `enabled == false`, return `"disabled"` regardless of health state.
- `stats.error_rate`: Compute `(total_errors / total_requests) * 100`. Return `0` if `total_requests == 0`.
- `stats.avg_latency_ms`: Track via exponential moving average (EMA with α=0.1) or sliding window.
- `stats.latency_history`: Keep last 12 latency samples per provider (ring buffer).

---

#### `GET /zz/api/providers/{name}`

Get single provider details. Same schema as one element of the `providers` array above.

**Response** `200 OK`: Single provider object.
**Response** `404 Not Found`: `{ "error": "Provider not found: {name}" }`

---

#### `POST /zz/api/providers`

Add a new provider at runtime.

**Request body**:
```jsonc
{
  "name": "ali-account-3",         // Required, must be unique
  "base_url": "https://...",       // Required
  "api_key": "sk-newkey",          // Required
  "priority": 4,                   // Optional, default: max_existing + 1
  "weight": 10,                    // Optional, default: 0
  "models": [],                    // Optional, default: []
  "headers": {},                   // Optional, default: {}
  "token_budget": null             // Optional, default: null
}
```

**Response** `201 Created`: The full provider object (same schema as GET).
**Response** `400 Bad Request`: `{ "error": "Provider name already exists" }`
**Response** `400 Bad Request`: `{ "error": "name is required" }`

**Side effects**:
- Add to `ProviderManager.providers` DashMap
- Append to in-memory config `provider_configs`
- Broadcast `provider_state` event via WebSocket (status: "healthy")
- **Does NOT persist to config.toml** (runtime-only until explicit config save)

---

#### `PUT /zz/api/providers/{name}`

Update an existing provider's configuration.

**Request body** (all fields optional, merge with existing):
```jsonc
{
  "base_url": "https://new-url.com",
  "api_key": "sk-newkey",
  "priority": 2,
  "weight": 30,
  "enabled": false,
  "models": ["model-a", "model-b"],
  "headers": { "X-New": "header" },
  "token_budget": 500000
}
```

**Response** `200 OK`: Updated full provider object.
**Response** `404 Not Found`: `{ "error": "Provider not found: {name}" }`

**Side effects**:
- Update `ProviderManager.providers` entry
- If `enabled` changed → broadcast `provider_state` via WebSocket
- **Does NOT persist to config.toml**

---

#### `DELETE /zz/api/providers/{name}`

Remove a provider at runtime.

**Response** `200 OK`: `{ "removed": "ali-account-3" }`
**Response** `404 Not Found`: `{ "error": "Provider not found: {name}" }`

**Side effects**:
- Remove from `ProviderManager.providers` DashMap
- Broadcast `provider_state` event via WebSocket (type: "removed")
- **Does NOT persist to config.toml**

---

#### `POST /zz/api/providers/{name}/test`

Test connectivity to a provider by sending a lightweight request.

**Implementation**: Send `GET {base_url}/v1/models` with the provider's API key. Measure latency.

**Response** `200 OK`:
```jsonc
{
  "success": true,
  "latency_ms": 350,
  "status_code": 200
}
```

**Response** `200 OK` (test failed, but API itself succeeded):
```jsonc
{
  "success": false,
  "latency_ms": 5000,
  "status_code": 401,
  "error": "Unauthorized"
}
```

**Response** `404 Not Found`: `{ "error": "Provider not found: {name}" }`

---

#### `POST /zz/api/providers/{name}/enable`

Enable a disabled provider.

**Response** `200 OK`: `{ "name": "...", "enabled": true }`
**Side effects**: Set `enabled = true`, set state to `Healthy`, broadcast via WS.

---

#### `POST /zz/api/providers/{name}/disable`

Disable a provider (stops receiving traffic).

**Response** `200 OK`: `{ "name": "...", "enabled": false }`
**Side effects**: Set `enabled = false`, broadcast via WS.

---

#### `POST /zz/api/providers/{name}/reset`

Reset a provider's health state (clear cooldown/unhealthy, zero failure counter).

**Response** `200 OK`: `{ "name": "...", "status": "healthy" }`
**Side effects**: Call `Provider::reset()`, broadcast via WS.

---

### 3.2 Routing

#### `GET /zz/api/routing`

Get current routing configuration.

**Response** `200 OK`:
```jsonc
{
  "strategy": "failover",
  "max_retries": 3,
  "cooldown_secs": 60,
  "failure_threshold": 3,
  "recovery_secs": 600,
  "pinned_provider": null,
  "model_rules": [
    { "id": "rule_1", "pattern": "qwen-*", "target_provider": "ali-account-1" },
    { "id": "rule_2", "pattern": "glm-*", "target_provider": "zhipu-account-1" }
  ]
}
```

---

#### `PUT /zz/api/routing`

Update routing strategy and/or parameters.

**Request body** (all fields optional, merge with existing):
```jsonc
{
  "strategy": "weighted-random",
  "max_retries": 5,
  "cooldown_secs": 120,
  "failure_threshold": 5,
  "recovery_secs": 300,
  "pinned_provider": "ali-account-1"
}
```

**Response** `200 OK`: Full updated routing config (same as GET response).

**Side effects**:
- Update `Router` strategy enum
- Update `HealthConfig` parameters
- Broadcast `stats` snapshot via WebSocket (includes new strategy)

---

#### `GET /zz/api/routing/rules`

Get model routing rules.

**Response** `200 OK`:
```jsonc
{
  "rules": [
    { "id": "rule_1", "pattern": "qwen-*", "target_provider": "ali-account-1" },
    { "id": "rule_2", "pattern": "glm-*", "target_provider": "zhipu-account-1" }
  ]
}
```

---

#### `PUT /zz/api/routing/rules`

Replace all model routing rules.

**Request body**:
```jsonc
{
  "rules": [
    { "pattern": "qwen-*", "target_provider": "ali-account-1" },
    { "pattern": "glm-*", "target_provider": "zhipu-account-1" }
  ]
}
```

**Response** `200 OK`: Updated rules list with generated IDs.

---

### 3.3 Stats

#### `GET /zz/api/stats`

Get aggregated system statistics.

**Response** `200 OK`:
```jsonc
{
  "total_requests": 12847,
  "requests_per_minute": 23.5,
  "active_providers": 3,
  "healthy_providers": 4,
  "total_providers": 5,
  "strategy": "failover",
  "uptime_secs": 86400
}
```

**Implementation notes**:
- `requests_per_minute`: Compute from a sliding 60-second window counter
- `active_providers`: Count where `enabled == true`
- `healthy_providers`: Count where `state == Healthy` (including recovered cooldown/unhealthy)
- `uptime_secs`: `Instant::now() - start_time`

---

#### `GET /zz/api/stats/timeseries?period=1h`

Time-series request rate data for chart rendering.

**Query params**:
- `period`: `1h` (default), `6h`, `24h`

**Response** `200 OK`:
```jsonc
{
  "period": "1h",
  "interval_secs": 60,
  "data": [
    { "time": "2026-03-21T12:05:00Z", "value": 23 },
    { "time": "2026-03-21T12:06:00Z", "value": 25 },
    // ... 60 data points for 1h at 1min intervals
  ]
}
```

**Implementation**: Maintain a ring buffer of per-minute request counts (at least 1440 entries for 24h).

---

### 3.4 Logs

#### `GET /zz/api/logs?limit=100&offset=0`

Get paginated request logs (newest first).

**Query params**:
- `limit`: Max entries to return (default: 100, max: 1000)
- `offset`: Skip N entries (default: 0)
- `status`: Filter by status class: `2xx`, `4xx`, `5xx`, `error` (optional)
- `provider`: Filter by provider name (optional)
- `search`: Keyword search in path/model/provider/id (optional)

**Response** `200 OK`:
```jsonc
{
  "logs": [
    {
      "id": "req_abc123",
      "timestamp": "2026-03-21T13:05:02Z",
      "method": "POST",
      "path": "/v1/chat/completions",
      "provider": "ali-account-1",
      "status": 200,
      "duration_ms": 2300,
      "ttfb_ms": 800,
      "model": "qwen-plus",
      "streaming": true,
      "request_bytes": 1200,
      "response_bytes": 3400,
      "failover_chain": null
    }
    // ...
  ],
  "total": 12847,
  "offset": 0,
  "limit": 100
}
```

**Implementation**: Store logs in a `VecDeque<LogEntry>` ring buffer capped at 10000 entries. Frontend keeps last 1000 in Zustand store.

---

### 3.5 Config

#### `GET /zz/api/config`

Get current config file content as raw TOML string.

**Response** `200 OK`:
```jsonc
{
  "content": "[server]\nlisten = \"127.0.0.1:9090\"\n...",
  "path": "/Users/xxx/.config/zz/config.toml",
  "last_modified": "2026-03-21T12:30:00Z",
  "last_reloaded": "2026-03-21T12:30:05Z"
}
```

---

#### `PUT /zz/api/config`

Validate, save to disk, and hot-reload configuration.

**Request body**:
```jsonc
{
  "content": "[server]\nlisten = \"127.0.0.1:9090\"\n..."
}
```

**Response** `200 OK`:
```jsonc
{
  "saved": true,
  "reloaded": true,
  "last_modified": "2026-03-21T14:00:00Z",
  "last_reloaded": "2026-03-21T14:00:01Z"
}
```

**Response** `400 Bad Request`:
```jsonc
{
  "saved": false,
  "error": "TOML parse error: expected `=`, found newline at line 5"
}
```

**Side effects**:
1. Parse TOML to validate
2. Write to disk (`config_path`)
3. Call `AppState::reload_config()`
4. Broadcast full `stats` snapshot via WS

---

#### `POST /zz/api/config/validate`

Validate TOML without saving.

**Request body**:
```jsonc
{
  "content": "[server]\nlisten = ..."
}
```

**Response** `200 OK`: `{ "valid": true }`
**Response** `200 OK`: `{ "valid": false, "error": "..." }`

---

### 3.6 System

#### `GET /zz/api/health`

**Response** `200 OK`:
```jsonc
{
  "status": "ok",
  "uptime_secs": 86400,
  "providers": [
    { "name": "ali-account-1", "status": "healthy" },
    { "name": "zhipu-account-1", "status": "cooldown" }
  ]
}
```

---

#### `GET /zz/api/version`

**Response** `200 OK`:
```jsonc
{
  "version": "0.1.0",
  "build_time": "2026-03-21T10:00:00Z",
  "rust_version": "1.82.0"
}
```

---

## 4. WebSocket Protocol

### Endpoint

`ws://127.0.0.1:9090/zz/ws`

Connection upgrade is via standard HTTP `Upgrade: websocket` handshake.

### Server → Client Messages

All messages are JSON with a `type` field for routing.

#### 4.1 `log` — Real-time request log entry

Sent immediately after each proxied request completes (success or failure).

```jsonc
{
  "type": "log",
  "data": {
    "id": "req_abc123",
    "timestamp": "2026-03-21T13:05:02Z",
    "method": "POST",
    "path": "/v1/chat/completions",
    "provider": "ali-account-1",
    "status": 200,
    "duration_ms": 2300,
    "ttfb_ms": 800,
    "model": "qwen-plus",
    "streaming": true,
    "request_bytes": 1200,
    "response_bytes": 3400,
    "failover_chain": null
  }
}
```

**`failover_chain`**: If retries occurred, an array of `"provider_name:status_code"` strings showing the full attempt chain, e.g. `["ali-1:429", "zhipu-1:200"]`. `null` if no failover.

**`model`**: Extracted from request body JSON field `"model"` (best-effort parse of first ~2KB). Use `"unknown"` if not parseable.

**`ttfb_ms`**: Time from sending upstream request to receiving first byte of response.

---

#### 4.2 `provider_state` — Provider state change

Sent when a provider's health state changes.

```jsonc
{
  "type": "provider_state",
  "data": {
    "name": "ali-account-1",
    "status": "cooldown",
    "cooldown_until": "2026-03-21T13:15:00Z",
    "consecutive_failures": 3,
    "enabled": true
  }
}
```

Triggers: mark_quota_exhausted, mark_failure (when threshold crossed), reset, enable, disable.

---

#### 4.3 `stats` — Periodic stats snapshot

Sent every **5 seconds** to all connected WebSocket clients.

```jsonc
{
  "type": "stats",
  "data": {
    "total_requests": 12847,
    "requests_per_minute": 23.5,
    "active_providers": 3,
    "healthy_providers": 4,
    "total_providers": 5,
    "strategy": "failover",
    "uptime_secs": 86400
  }
}
```

---

### Client → Server Messages

#### 4.4 `subscribe` — Event type filter (optional)

By default, all event types are sent. Client may narrow:

```jsonc
{
  "type": "subscribe",
  "events": ["log", "stats"]
}
```

Valid event types: `"log"`, `"provider_state"`, `"stats"`.

---

### Connection Management

- **Auto-reconnect**: Frontend should reconnect on close with exponential backoff (1s, 2s, 4s, 8s, max 30s)
- **Ping/Pong**: Backend sends WebSocket ping every 30s; client responds with pong
- **Max clients**: No hard limit; use `tokio::sync::broadcast` channel (drop slow consumers)

---

## 5. Static File Serving

### Development Mode

Vite dev server at `localhost:5173` proxies API calls to backend:

```ts
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      '/zz/api': 'http://127.0.0.1:9090',
      '/zz/ws': { target: 'ws://127.0.0.1:9090', ws: true },
    }
  }
});
```

With this config, CORS is not needed in dev mode (Vite proxies). CORS on the backend is still recommended for flexibility.

### Production Mode

Backend serves `ui/dist/**` files at `/zz/ui/*`:

- `GET /zz/ui/` → `index.html`
- `GET /zz/ui/assets/*` → bundled JS/CSS
- Use `rust-embed` or `include_dir` crate to embed at compile time

---

## 6. Error Response Format

All API errors follow a consistent format:

```jsonc
{
  "error": "Human-readable error message"
}
```

HTTP status codes:
- `200` — Success
- `201` — Created (POST new resource)
- `204` — No Content (OPTIONS preflight, DELETE with no body)
- `400` — Bad Request (validation error, malformed JSON)
- `404` — Not Found (resource doesn't exist)
- `500` — Internal Server Error

---

## 7. Request ID Generation

Each proxied request gets a unique ID: `req_{nanoid(12)}`.

Format: `req_` prefix + 12-char alphanumeric random string.

Implementation: Use `rand` to generate. Example: `req_a1b2c3d4e5f6`.

---

## 8. Data Collection Requirements (Backend)

To support the API and WebSocket, the backend must collect:

| Data | Storage | Retention |
|------|---------|-----------|
| Per-request log entries | `VecDeque<LogEntry>` | Last 10,000 entries |
| Per-provider latency samples | `VecDeque<u64>` per provider | Last 12 samples |
| Per-provider request/error counts | `AtomicU64` | Lifetime (reset on restart) |
| Per-minute request rate | Ring buffer `[u32; 1440]` | Last 24 hours |
| Server start time | `Instant` | Lifetime |
| Provider enabled flags | `AtomicBool` per provider | Lifetime |
| Model routing rules | `RwLock<Vec<ModelRule>>` | Lifetime |

---

## 9. Compatibility with Existing Endpoints

The existing admin endpoints (`/zz/health`, `/zz/stats`, `/zz/reload`) should be **kept** for backward compatibility and CLI/curl usage. The new `/zz/api/*` endpoints are the primary interface for the UI.

| Legacy | New Equivalent |
|--------|---------------|
| `GET /zz/health` | `GET /zz/api/health` |
| `GET /zz/stats` | `GET /zz/api/stats` |
| `POST /zz/reload` | `PUT /zz/api/config` (save + reload) |
