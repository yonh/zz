# ZZ - Frontend-Backend Integration Implementation Guide

This document provides file-level implementation instructions for connecting the existing Rust backend with the React frontend dashboard. It references `SPEC-API.md` for the API contract.

**Prerequisite**: Read `SPEC-API.md` for the complete API and WebSocket specification.

---

## Implementation Phases

```
Phase 0: Foundation (CORS + request routing + shared types)     ~2h
Phase 1: Data Collection (structured logs + latency + rpm)      ~3h
Phase 2: REST API Endpoints (/zz/api/*)                         ~4h
Phase 3: WebSocket (real-time push)                             ~3h
Phase 4: Frontend API Client (replace mock data)                ~2h
Phase 5: Backend feature gaps (strategies + model rules)        ~2h
                                                         Total: ~16h
```

---

## Phase 0: Foundation

### 0.1 Add Dependencies

**File**: `Cargo.toml`

Add WebSocket, JSON serialization, and ID generation support:

```toml
[dependencies]
# ... existing deps ...
tokio-tungstenite = "0.26"        # WebSocket support
uuid = { version = "1.0", features = ["v4"] }  # Request ID generation (or use nanoid)
```

> `serde_json` is already present. `tokio` already has `sync` feature (for broadcast channel).

---

### 0.2 Request Router Refactor

**File**: `src/main.rs` → modify the service_fn closure

Currently `admin::handle_admin_request` checks `/zz/` prefix and handles 3 endpoints. Refactor to a proper path router:

```
/zz/api/*        → admin_api::handle_api_request()     [NEW]
/zz/ws           → ws::handle_ws_upgrade()              [NEW]
/zz/ui/*         → static_files::serve()                [NEW, Phase 5]
/zz/health       → admin::handle_health()               [KEEP legacy]
/zz/stats        → admin::handle_stats()                [KEEP legacy]
/zz/reload       → admin::handle_reload()               [KEEP legacy]
/*               → proxy::proxy_handler()               [KEEP]
```

**Changes to `src/main.rs`**:

```rust
// In the service_fn closure, replace the current routing:

// 1. Check new API endpoints
if path.starts_with("/zz/api/") {
    return admin_api::handle_api_request(req, &state).await;
}

// 2. Check WebSocket upgrade
if path == "/zz/ws" {
    return ws::handle_ws_upgrade(req, &state).await;
}

// 3. Legacy admin endpoints (keep for backward compat)
if let Some(resp) = admin::handle_admin_request(&req, Some(&state)) {
    return Ok(resp);
}

// 4. CORS preflight for /zz/* paths
if req.method() == Method::OPTIONS && path.starts_with("/zz/") {
    return Ok(cors_preflight_response());
}

// 5. Proxy
proxy::proxy_handler(req, state).await
```

---

### 0.3 CORS Helper

**File**: `src/cors.rs` [NEW]

```rust
use hyper::{Response, StatusCode, header};

/// Add CORS headers to any response.
pub fn add_cors_headers(resp: &mut Response<impl hyper::body::Body>) {
    let headers = resp.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization".parse().unwrap());
    headers.insert(header::ACCESS_CONTROL_MAX_AGE, "86400".parse().unwrap());
}

/// Return a 204 No Content response for OPTIONS preflight.
pub fn preflight_response() -> Response<ResponseBody> {
    let mut resp = Response::new(empty_body());
    *resp.status_mut() = StatusCode::NO_CONTENT;
    add_cors_headers(&mut resp);
    resp
}
```

All `/zz/api/*` handlers MUST call `add_cors_headers(&mut resp)` before returning.

---

### 0.4 Shared AppState Extension

**File**: `src/proxy.rs` → `AppState` struct

Add fields for new subsystems:

```rust
#[derive(Clone)]
pub struct AppState {
    pub provider_manager: Arc<crate::provider::ProviderManager>,
    pub router: Arc<crate::router::Router>,
    pub config: Arc<std::sync::RwLock<crate::config::Config>>,
    pub config_path: String,

    // NEW fields for UI integration:
    pub start_time: std::time::Instant,                           // For uptime_secs
    pub log_buffer: Arc<crate::logging::RequestLogBuffer>,        // Structured request logs
    pub ws_broadcaster: Arc<crate::ws::WsBroadcaster>,            // WebSocket broadcast channel
    pub model_rules: Arc<std::sync::RwLock<Vec<crate::router::ModelRule>>>,  // Model routing rules
    pub rpm_counter: Arc<crate::stats::RpmCounter>,               // Requests per minute
}
```

---

## Phase 1: Data Collection

### 1.1 Structured Request Log

**File**: `src/logging.rs` → add `RequestLogBuffer`

Each proxied request produces a `LogEntry`. Store in a thread-safe ring buffer.

```rust
use std::collections::VecDeque;
use std::sync::Mutex;
use serde::Serialize;

/// A single proxied request log entry.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: String,                      // "req_XXXXXXXXXXXX"
    pub timestamp: String,               // ISO 8601
    pub method: String,
    pub path: String,
    pub provider: String,
    pub status: u16,
    pub duration_ms: u64,
    pub ttfb_ms: u64,
    pub model: String,
    pub streaming: bool,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub failover_chain: Option<Vec<String>>,  // ["ali-1:429", "zhipu-1:200"]
}

/// Thread-safe ring buffer for request logs.
pub struct RequestLogBuffer {
    entries: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl RequestLogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push a new log entry (drops oldest if at capacity).
    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.entries.lock().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_back();
        }
        buf.push_front(entry);
    }

    /// Get paginated entries (newest first).
    pub fn get_page(&self, offset: usize, limit: usize) -> (Vec<LogEntry>, usize) {
        let buf = self.entries.lock().unwrap();
        let total = buf.len();
        let entries: Vec<LogEntry> = buf.iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        (entries, total)
    }
}
```

---

### 1.2 Collect Logs in Proxy Handler

**File**: `src/proxy.rs` → `proxy_handler()` function

After request completes (success or all-providers-failed), build and store a `LogEntry`:

```rust
// At top of proxy_handler, before the retry loop:
let request_id = format!("req_{}", generate_id());  // 12-char random
let start_time = std::time::Instant::now();
let request_bytes = body_bytes.len() as u64;
let mut failover_chain: Vec<String> = Vec::new();

// Inside the retry loop, after each attempt:
failover_chain.push(format!("{}:{}", provider_name, status_code));

// After the loop (on success or final failure), build LogEntry:
let log_entry = LogEntry {
    id: request_id,
    timestamp: chrono::Utc::now().to_rfc3339(),
    method: method.to_string(),
    path: path.clone(),
    provider: final_provider_name,
    status: final_status_code,
    duration_ms: start_time.elapsed().as_millis() as u64,
    ttfb_ms: ttfb_duration.as_millis() as u64,
    model: extract_model(&body_bytes),
    streaming: is_sse,
    request_bytes,
    response_bytes,
    failover_chain: if failover_chain.len() > 1 { Some(failover_chain) } else { None },
};

state.log_buffer.push(log_entry.clone());
state.ws_broadcaster.broadcast_log(&log_entry);
state.rpm_counter.increment();
```

**Model extraction** helper:

```rust
/// Best-effort extract "model" field from request body JSON.
fn extract_model(body: &[u8]) -> String {
    // Only check first 2KB
    let check = &body[..body.len().min(2048)];
    if let Ok(s) = std::str::from_utf8(check) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
                return m.to_string();
            }
        }
    }
    "unknown".to_string()
}
```

---

### 1.3 Per-Provider Latency Tracking

**File**: `src/provider.rs` → `Provider` struct

Add latency tracking and enabled flag:

```rust
pub struct Provider {
    pub config: crate::config::ProviderConfig,
    pub state: std::sync::Mutex<ProviderState>,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub failure_count: std::sync::Mutex<usize>,

    // NEW:
    pub enabled: std::sync::atomic::AtomicBool,        // Runtime enable/disable
    pub latency_history: Mutex<VecDeque<u64>>,          // Last 12 latency samples (ms)
    pub latency_ema: Mutex<f64>,                        // Exponential moving average
}
```

Add methods:

```rust
impl Provider {
    /// Record a request latency sample.
    pub fn record_latency(&self, latency_ms: u64) {
        let mut history = self.latency_history.lock().unwrap();
        if history.len() >= 12 {
            history.pop_front();
        }
        history.push_back(latency_ms);

        let mut ema = self.latency_ema.lock().unwrap();
        if *ema == 0.0 {
            *ema = latency_ms as f64;
        } else {
            *ema = *ema * 0.9 + latency_ms as f64 * 0.1;
        }
    }

    /// Check if provider is enabled and available.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::Relaxed)
    }
}
```

Update `ProviderManager::get_available()` to also filter by `enabled`:

```rust
pub fn get_available(&self) -> Vec<(String, Arc<Provider>)> {
    self.providers
        .iter()
        .filter(|entry| entry.value().is_enabled() && entry.value().is_available())
        .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
        .collect()
}
```

---

### 1.4 Requests-Per-Minute Counter

**File**: `src/stats.rs` [NEW]

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// Sliding window counter for requests per minute.
pub struct RpmCounter {
    /// Per-second counters for the current minute.
    buckets: [AtomicU32; 60],
    /// Current second index (0-59).
    current_idx: Mutex<usize>,
}

impl RpmCounter {
    pub fn new() -> Self { /* ... */ }

    /// Called on each request.
    pub fn increment(&self) { /* increment current second's bucket */ }

    /// Get requests in the last 60 seconds.
    pub fn get_rpm(&self) -> f64 { /* sum all 60 buckets */ }

    /// Background task: advance the second pointer every second, zeroing the next bucket.
    pub async fn run_ticker(&self) { /* tokio::time::interval(1s) loop */ }
}
```

---

## Phase 2: REST API Endpoints

### 2.1 New Module Structure

**File**: `src/admin_api.rs` [NEW]

Main router for all `/zz/api/*` endpoints:

```rust
pub async fn handle_api_request(
    req: hyper::Request<hyper::body::Incoming>,
    state: &AppState,
) -> Result<hyper::Response<ResponseBody>, hyper::Error> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    let mut resp = match (method, path.as_str()) {
        // Providers
        (Method::GET,    "/zz/api/providers")              => handle_list_providers(state).await,
        (Method::POST,   "/zz/api/providers")              => handle_add_provider(req, state).await,
        (Method::GET,    p) if p.starts_with("/zz/api/providers/") && !p.contains("/test") && !p.contains("/enable") && !p.contains("/disable") && !p.contains("/reset")
            => handle_get_provider(p, state).await,
        (Method::PUT,    p) if p.starts_with("/zz/api/providers/") && p.matches('/').count() == 4
            => handle_update_provider(req, p, state).await,
        (Method::DELETE, p) if p.starts_with("/zz/api/providers/")
            => handle_delete_provider(p, state).await,
        (Method::POST,   p) if p.ends_with("/test")
            => handle_test_provider(p, state).await,
        (Method::POST,   p) if p.ends_with("/enable")
            => handle_enable_provider(p, state).await,
        (Method::POST,   p) if p.ends_with("/disable")
            => handle_disable_provider(p, state).await,
        (Method::POST,   p) if p.ends_with("/reset")
            => handle_reset_provider(p, state).await,

        // Routing
        (Method::GET,    "/zz/api/routing")                => handle_get_routing(state).await,
        (Method::PUT,    "/zz/api/routing")                => handle_update_routing(req, state).await,
        (Method::GET,    "/zz/api/routing/rules")          => handle_get_rules(state).await,
        (Method::PUT,    "/zz/api/routing/rules")          => handle_update_rules(req, state).await,

        // Stats
        (Method::GET,    "/zz/api/stats")                  => handle_get_stats(state).await,
        (Method::GET,    "/zz/api/stats/timeseries")       => handle_get_timeseries(req, state).await,

        // Logs
        (Method::GET,    "/zz/api/logs")                   => handle_get_logs(req, state).await,

        // Config
        (Method::GET,    "/zz/api/config")                 => handle_get_config(state).await,
        (Method::PUT,    "/zz/api/config")                 => handle_update_config(req, state).await,
        (Method::POST,   "/zz/api/config/validate")        => handle_validate_config(req).await,

        // System
        (Method::GET,    "/zz/api/health")                 => handle_health(state).await,
        (Method::GET,    "/zz/api/version")                => handle_version().await,

        _ => json_error_response(StatusCode::NOT_FOUND, "Endpoint not found"),
    };

    // Add CORS headers to every response
    crate::cors::add_cors_headers(&mut resp);
    Ok(resp)
}
```

### 2.2 Provider API Handler Details

Each handler follows the pattern:

```rust
async fn handle_list_providers(state: &AppState) -> Response<ResponseBody> {
    let providers: Vec<ProviderJson> = state.provider_manager
        .get_all_full()     // NEW method returning config + state + stats
        .into_iter()
        .map(|p| ProviderJson::from(p))
        .collect();

    json_response(StatusCode::OK, &serde_json::json!({ "providers": providers }))
}
```

**Key**: The backend `ProviderManager` needs a new method `get_all_full()` that returns a combined view of config + runtime state + stats + latency history for each provider.

### 2.3 JSON Response Helpers

```rust
fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response<ResponseBody> {
    let json = serde_json::to_string(body).unwrap();
    let mut resp = Response::new(full(json));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}

fn json_error_response(status: StatusCode, message: &str) -> Response<ResponseBody> {
    json_response(status, &serde_json::json!({ "error": message }))
}
```

---

## Phase 3: WebSocket

### 3.1 WebSocket Broadcaster

**File**: `src/ws.rs` [NEW]

```rust
use tokio::sync::broadcast;
use serde::Serialize;

/// Events that can be broadcast to WebSocket clients.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    #[serde(rename = "log")]
    Log(crate::logging::LogEntry),

    #[serde(rename = "provider_state")]
    ProviderState(ProviderStateEvent),

    #[serde(rename = "stats")]
    Stats(StatsSnapshot),
}

#[derive(Clone, Debug, Serialize)]
pub struct ProviderStateEvent {
    pub name: String,
    pub status: String,
    pub cooldown_until: Option<String>,
    pub consecutive_failures: usize,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatsSnapshot {
    pub total_requests: u64,
    pub requests_per_minute: f64,
    pub active_providers: usize,
    pub healthy_providers: usize,
    pub total_providers: usize,
    pub strategy: String,
    pub uptime_secs: u64,
}

/// Manages broadcast channel for WebSocket clients.
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// Broadcast a JSON event to all connected clients.
    pub fn broadcast(&self, event: &WsEvent) {
        let json = serde_json::to_string(event).unwrap();
        let _ = self.tx.send(json);  // Ignore error if no receivers
    }

    /// Get a receiver for a new WebSocket client.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    // Convenience methods
    pub fn broadcast_log(&self, entry: &crate::logging::LogEntry) {
        self.broadcast(&WsEvent::Log(entry.clone()));
    }

    pub fn broadcast_provider_state(&self, event: ProviderStateEvent) {
        self.broadcast(&WsEvent::ProviderState(event));
    }

    pub fn broadcast_stats(&self, snapshot: StatsSnapshot) {
        self.broadcast(&WsEvent::Stats(snapshot));
    }
}
```

### 3.2 WebSocket Upgrade Handler

**File**: `src/ws.rs` → `handle_ws_upgrade()`

Handle HTTP → WebSocket upgrade, then spawn a task to forward broadcast messages to the client:

```rust
pub async fn handle_ws_upgrade(
    req: hyper::Request<hyper::body::Incoming>,
    state: &AppState,
) -> Result<hyper::Response<ResponseBody>, hyper::Error> {
    // Verify Upgrade header
    // Use tokio-tungstenite to complete the upgrade
    // Spawn a task that:
    //   1. Subscribes to ws_broadcaster
    //   2. Loops: recv from broadcast → send to WS client
    //   3. Also handles client→server messages (subscribe filter)
    //   4. Sends ping every 30s
}
```

### 3.3 Periodic Stats Broadcast

**File**: `src/main.rs` → spawn a background task after server setup:

```rust
// Spawn stats broadcaster (every 5 seconds)
let state_clone = state.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        let snapshot = build_stats_snapshot(&state_clone);
        state_clone.ws_broadcaster.broadcast_stats(snapshot);
    }
});
```

---

## Phase 4: Frontend API Client

### 4.1 Create `api/client.ts`

**File**: `ui/src/api/client.ts` [NEW]

Replace mock data with real API calls:

```typescript
const BASE = '/zz/api';

/** Typed fetch wrapper with error handling. */
async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!resp.ok) {
    const err = await resp.json().catch(() => ({ error: resp.statusText }));
    throw new Error(err.error || resp.statusText);
  }
  return resp.json();
}

// --- Providers ---
export const api = {
  getProviders: () => apiFetch<{ providers: Provider[] }>('/providers'),
  getProvider: (name: string) => apiFetch<Provider>(`/providers/${name}`),
  addProvider: (data: Partial<Provider>) => apiFetch<Provider>('/providers', { method: 'POST', body: JSON.stringify(data) }),
  updateProvider: (name: string, data: Partial<Provider>) => apiFetch<Provider>(`/providers/${name}`, { method: 'PUT', body: JSON.stringify(data) }),
  deleteProvider: (name: string) => apiFetch<void>(`/providers/${name}`, { method: 'DELETE' }),
  testProvider: (name: string) => apiFetch<{ success: boolean; latency_ms: number }>(`/providers/${name}/test`, { method: 'POST' }),
  enableProvider: (name: string) => apiFetch<void>(`/providers/${name}/enable`, { method: 'POST' }),
  disableProvider: (name: string) => apiFetch<void>(`/providers/${name}/disable`, { method: 'POST' }),

  // --- Routing ---
  getRouting: () => apiFetch<RoutingConfig & { model_rules: ModelRule[] }>('/routing'),
  updateRouting: (data: Partial<RoutingConfig>) => apiFetch<RoutingConfig>('/routing', { method: 'PUT', body: JSON.stringify(data) }),
  getRules: () => apiFetch<{ rules: ModelRule[] }>('/routing/rules'),
  updateRules: (rules: ModelRule[]) => apiFetch<{ rules: ModelRule[] }>('/routing/rules', { method: 'PUT', body: JSON.stringify({ rules }) }),

  // --- Stats ---
  getStats: () => apiFetch<SystemStats>('/stats'),
  getTimeseries: (period: string) => apiFetch<{ data: TimeSeriesPoint[] }>(`/stats/timeseries?period=${period}`),

  // --- Logs ---
  getLogs: (params?: { limit?: number; offset?: number; status?: string; provider?: string; search?: string }) => {
    const qs = new URLSearchParams(params as Record<string, string>).toString();
    return apiFetch<{ logs: LogEntry[]; total: number }>(`/logs?${qs}`);
  },

  // --- Config ---
  getConfig: () => apiFetch<{ content: string; path: string; last_modified: string; last_reloaded: string }>('/config'),
  updateConfig: (content: string) => apiFetch<{ saved: boolean; reloaded: boolean }>('/config', { method: 'PUT', body: JSON.stringify({ content }) }),
  validateConfig: (content: string) => apiFetch<{ valid: boolean; error?: string }>('/config/validate', { method: 'POST', body: JSON.stringify({ content }) }),

  // --- System ---
  getHealth: () => apiFetch<{ status: string }>('/health'),
};
```

### 4.2 Replace Mock WebSocket with Real WebSocket

**File**: `ui/src/hooks/useWebSocket.ts` [NEW, replaces `useMockWebSocket.ts`]

```typescript
import { useEffect, useRef, useCallback } from 'react';
import { useAppStore } from '@/stores/store';
import type { LogEntry } from '@/api/types';

/**
 * Real WebSocket hook connecting to /zz/ws with auto-reconnect.
 */
export function useWebSocket() {
  const wsRef = useRef<WebSocket | null>(null);
  const retryRef = useRef(0);

  const addLog = useAppStore((s) => s.addLog);
  const updateProviderStatus = useAppStore((s) => s.updateProviderStatus);
  const setSystemStats = useAppStore((s) => s.setSystemStats);  // NEW action needed

  const connect = useCallback(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/zz/ws`);

    ws.onopen = () => {
      retryRef.current = 0;
      console.log('[WS] Connected');
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        switch (msg.type) {
          case 'log':
            addLog(msg.data as LogEntry);
            break;
          case 'provider_state':
            updateProviderStatus(msg.data.name, msg.data.status, msg.data.cooldown_until);
            break;
          case 'stats':
            setSystemStats(msg.data);
            break;
        }
      } catch (e) {
        console.error('[WS] Parse error:', e);
      }
    };

    ws.onclose = () => {
      const delay = Math.min(1000 * Math.pow(2, retryRef.current), 30000);
      retryRef.current++;
      console.log(`[WS] Disconnected. Reconnecting in ${delay}ms...`);
      setTimeout(connect, delay);
    };

    wsRef.current = ws;
  }, [addLog, updateProviderStatus, setSystemStats]);

  useEffect(() => {
    connect();
    return () => wsRef.current?.close();
  }, [connect]);
}
```

### 4.3 Store Initialization from REST API

**File**: `ui/src/stores/store.ts` → modify initialization

Replace mock data imports with an `initFromApi()` action:

```typescript
// Remove: import { mockProviders, mockSystemStats, ... } from "@/api/mock";
// Add:
import { api } from "@/api/client";

// Add to AppState interface:
setSystemStats: (stats: SystemStats) => void;
initFromApi: () => Promise<void>;

// Add to create():
setSystemStats: (stats) => set({ systemStats: stats }),

initFromApi: async () => {
  try {
    const [providersRes, statsRes, routingRes, configRes, logsRes] = await Promise.all([
      api.getProviders(),
      api.getStats(),
      api.getRouting(),
      api.getConfig(),
      api.getLogs({ limit: 1000 }),
    ]);
    set({
      providers: providersRes.providers,
      systemStats: statsRes,
      routingConfig: {
        strategy: routingRes.strategy,
        max_retries: routingRes.max_retries,
        cooldown_secs: routingRes.cooldown_secs,
        failure_threshold: routingRes.failure_threshold,
        recovery_secs: routingRes.recovery_secs,
        pinned_provider: routingRes.pinned_provider,
      },
      modelRules: routingRes.model_rules || [],
      configToml: configRes.content,
      logs: logsRes.logs,
    });
  } catch (e) {
    console.error('Failed to load initial data from API:', e);
    // Fall back to mock data for development
  }
},
```

### 4.4 Wire User Actions to REST API

**File**: `ui/src/stores/store.ts` → modify action handlers

Each user mutation should: (1) call REST API, (2) update local store on success, (3) show toast.

Example for `toggleProvider`:

```typescript
// Before (mock-only):
toggleProvider: (name) => set((state) => ({ providers: ... })),

// After (API-backed):
toggleProvider: async (name) => {
  const provider = get().providers.find(p => p.name === name);
  if (!provider) return;

  try {
    if (provider.enabled) {
      await api.disableProvider(name);
    } else {
      await api.enableProvider(name);
    }
    // Optimistic update (WS will also push state change)
    set((state) => ({
      providers: state.providers.map((p) =>
        p.name === name ? { ...p, enabled: !p.enabled, status: !p.enabled ? "healthy" : "disabled" } : p
      ),
    }));
  } catch (e) {
    toast.error(`Failed to toggle provider: ${e.message}`);
  }
},
```

Apply the same pattern to: `setStrategy`, `updateProvider`, `addProvider`, `removeProvider`, `setRoutingConfig`, `addModelRule`, `removeModelRule`.

### 4.5 Vite Proxy Configuration

**File**: `ui/vite.config.ts`

Add proxy rules for development:

```typescript
export default defineConfig({
  // ...
  server: {
    proxy: {
      '/zz/api': {
        target: 'http://127.0.0.1:9090',
        changeOrigin: true,
      },
      '/zz/ws': {
        target: 'ws://127.0.0.1:9090',
        ws: true,
      },
    },
  },
});
```

---

## Phase 5: Backend Feature Gaps

### 5.1 Add `quota-aware` and `manual` Routing Strategies

**File**: `src/router.rs`

```rust
pub enum RoutingStrategy {
    Failover,
    RoundRobin,
    WeightedRandom,
    QuotaAware,     // NEW
    Manual,         // NEW
}

// In select_provider():
RoutingStrategy::Manual => {
    // Read pinned_provider from config
    let pinned = state.config.read().unwrap().routing.pinned_provider.clone();
    if let Some(name) = pinned {
        providers.iter().find(|(n, _)| n == &name).cloned()
    } else {
        None  // No provider pinned
    }
}

RoutingStrategy::QuotaAware => {
    // Select provider with lowest usage percentage relative to token_budget
    // Requires token_budget in ProviderConfig and usage tracking
    // Fallback to failover if no budgets configured
}
```

### 5.2 Add `enabled` Field to Provider Config

**File**: `src/config.rs` → `ProviderConfig`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub priority: usize,
    #[serde(default)]
    pub weight: usize,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub token_budget: Option<u64>,  // NEW

    // Note: `enabled` is a runtime state, not persisted in config.
    // It lives on Provider struct as AtomicBool, default true.
}
```

### 5.3 Add `pinned_provider` to Routing Config

**File**: `src/config.rs` → `RoutingConfig`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    #[serde(default = "default_routing_strategy")]
    pub strategy: String,
    #[serde(default = "default_retry_on_failure")]
    pub retry_on_failure: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default)]
    pub pinned_provider: Option<String>,  // NEW: for manual strategy
}
```

### 5.4 Model Routing Rules

**File**: `src/router.rs` → add `ModelRule` and matching logic

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelRule {
    pub id: String,
    pub pattern: String,          // Glob pattern: "qwen-*", "glm-*"
    pub target_provider: String,
}

impl Router {
    /// Check model rules before applying global strategy.
    pub fn select_provider_for_model(
        &self,
        model: &str,
        rules: &[ModelRule],
        providers: &[(String, Arc<Provider>)],
    ) -> Option<(String, Arc<Provider>)> {
        for rule in rules {
            if glob_match(&rule.pattern, model) {
                return providers.iter()
                    .find(|(name, _)| name == &rule.target_provider)
                    .cloned();
            }
        }
        None  // No rule matched, fall through to global strategy
    }
}
```

---

## New Module File Listing

After implementation, the `src/` directory should be:

```
src/
├── main.rs              # Entry point, server, background tasks
├── config.rs            # Config parsing (updated: pinned_provider, token_budget)
├── proxy.rs             # Proxy handler (updated: log collection, model extraction)
├── router.rs            # Routing logic (updated: QuotaAware, Manual, ModelRule)
├── provider.rs          # Provider state (updated: enabled, latency tracking)
├── rewriter.rs          # URL/header rewriting (unchanged)
├── stream.rs            # SSE utilities (unchanged)
├── error.rs             # Error types (unchanged)
├── logging.rs           # Logging + RequestLogBuffer (updated)
├── admin.rs             # Legacy /zz/* endpoints (unchanged, kept for compat)
├── admin_api.rs         # NEW: /zz/api/* REST endpoints
├── ws.rs                # NEW: WebSocket handler + broadcaster
├── cors.rs              # NEW: CORS middleware
└── stats.rs             # NEW: RPM counter + stats aggregation
```

---

## Testing Checklist

### Backend Verification

```bash
# 1. Health check
curl http://127.0.0.1:9090/zz/api/health

# 2. List providers
curl http://127.0.0.1:9090/zz/api/providers | jq

# 3. Get stats
curl http://127.0.0.1:9090/zz/api/stats | jq

# 4. Get routing config
curl http://127.0.0.1:9090/zz/api/routing | jq

# 5. Update strategy
curl -X PUT http://127.0.0.1:9090/zz/api/routing \
  -H "Content-Type: application/json" \
  -d '{"strategy": "round-robin"}'

# 6. Get logs
curl "http://127.0.0.1:9090/zz/api/logs?limit=10" | jq

# 7. Get config
curl http://127.0.0.1:9090/zz/api/config | jq

# 8. Test WebSocket
websocat ws://127.0.0.1:9090/zz/ws

# 9. CORS preflight
curl -X OPTIONS http://127.0.0.1:9090/zz/api/providers -i

# 10. Add provider
curl -X POST http://127.0.0.1:9090/zz/api/providers \
  -H "Content-Type: application/json" \
  -d '{"name":"test-provider","base_url":"https://api.example.com","api_key":"sk-test"}'
```

### Frontend Verification

```bash
cd ui && pnpm dev
# Open http://localhost:5173
# Verify:
# - Overview page loads with real stats
# - Providers page shows backend providers
# - Logs page receives real-time entries via WebSocket
# - Strategy change persists to backend
# - Provider edit calls REST API
```

### Integration Smoke Test

```bash
# 1. Start backend
cargo run -- --config config.toml.example

# 2. Start frontend
cd ui && pnpm dev

# 3. Send a proxy request
curl -X POST http://127.0.0.1:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer dummy" \
  -d '{"model":"qwen-plus","messages":[{"role":"user","content":"hi"}]}'

# 4. Verify in UI:
#    - New log entry appears in Logs page (via WebSocket)
#    - Stats update on Overview page
#    - Provider stats update on Providers page
```
