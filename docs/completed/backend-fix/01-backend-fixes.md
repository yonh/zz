# ZZ - Backend Integration Fix Guide

This document provides precise, file-level instructions to fix the 6 blocking issues preventing full frontend-backend connectivity. Each fix includes the exact file, location, and code changes needed.

**Prerequisites**: Read `../admin-api/01-api-spec.md` for the API contract and `../integration/01-integration-spec.md` for the overall architecture.

---

## Fix Priority Order

```
Fix 1: [P0] admin_api.rs — Routing rules path mismatch          ~5min
Fix 2: [P0] admin_api.rs — Stats use real rpm_counter + uptime   ~10min
Fix 3: [P0] admin_api.rs — Logs use real log_buffer              ~5min
Fix 4: [P0] proxy.rs — Integrate log collection pipeline         ~1h
Fix 5: [P0] main.rs + ws.rs — WebSocket upgrade + routing        ~2h
Fix 6: [P1] provider.rs + admin_api.rs — Add/Delete/Update impl  ~1h
                                                          Total: ~4.5h
```

---

## Fix 1: Routing Rules Path Mismatch

**Problem**: Backend registers `/zz/api/rules`, frontend requests `/zz/api/routing/rules`.

**File**: `src/admin_api.rs`

**Lines 133-136** — Change route paths:

```rust
// BEFORE (WRONG):
("/zz/api/rules", &hyper::Method::GET) => handle_get_rules(&state).await,
("/zz/api/rules", &hyper::Method::PUT) => {
    handle_update_rules(req, &state).await
}

// AFTER (CORRECT):
("/zz/api/routing/rules", &hyper::Method::GET) => handle_get_rules(&state).await,
("/zz/api/routing/rules", &hyper::Method::PUT) => {
    handle_update_rules(req, &state).await
}
```

Additionally, the `handle_get_rules` and `handle_update_rules` handlers should use `state.model_rules` instead of returning empty arrays:

**`handle_get_rules`** (~line 507):

```rust
async fn handle_get_rules(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let rules = state.model_rules.read().unwrap();
    let rules_json: Vec<serde_json::Value> = rules.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "pattern": r.pattern,
            "target_provider": r.target_provider
        })
    }).collect();
    json_response(&serde_json::json!({ "rules": rules_json }))
}
```

**`handle_update_rules`** (~line 511):

```rust
async fn handle_update_rules(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
    };

    #[derive(serde::Deserialize)]
    struct RuleInput {
        pattern: String,
        target_provider: String,
    }

    #[derive(serde::Deserialize)]
    struct UpdateRulesRequest {
        rules: Vec<RuleInput>,
    }

    let update_req: UpdateRulesRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

    // Convert to ModelRule with generated IDs
    let new_rules: Vec<crate::router::ModelRule> = update_req.rules.into_iter()
        .enumerate()
        .map(|(i, r)| crate::router::ModelRule {
            id: format!("rule_{}", i + 1),
            pattern: r.pattern,
            target_provider: r.target_provider,
        })
        .collect();

    // Store in state
    let rules_json: Vec<serde_json::Value> = new_rules.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "pattern": r.pattern,
            "target_provider": r.target_provider
        })
    }).collect();

    *state.model_rules.write().unwrap() = new_rules;

    json_response(&serde_json::json!({ "rules": rules_json }))
}
```

---

## Fix 2: Stats Use Real RPM + Uptime

**Problem**: `handle_get_stats` and `handle_health` hardcode `requests_per_minute: 0.0` and `uptime_secs: 0` despite `state.rpm_counter` and `state.start_time` being available.

**File**: `src/admin_api.rs`

**`handle_get_stats`** (~line 539):

```rust
// BEFORE:
"requests_per_minute": 0.0,
"uptime_secs": 0

// AFTER:
"requests_per_minute": state.rpm_counter.get_rpm(),
"uptime_secs": state.start_time.elapsed().as_secs()
```

**`handle_health`** (~line 719):

```rust
// BEFORE:
"uptime_secs": 0,

// AFTER:
"uptime_secs": state.start_time.elapsed().as_secs(),
```

---

## Fix 3: Logs Use Real log_buffer

**Problem**: `handle_get_logs` returns empty array instead of using `state.log_buffer`.

**File**: `src/admin_api.rs`

**`handle_get_logs`** (~line 576):

```rust
async fn handle_get_logs(
    uri: &hyper::Uri,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    let params = parse_query_params(uri);
    let offset: usize = params.get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let limit: usize = params.get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
        .min(1000);

    let logs = state.log_buffer.get_page(offset, limit);
    let total = state.log_buffer.len();

    json_response(&serde_json::json!({
        "logs": logs,
        "total": total,
        "offset": offset,
        "limit": limit
    }))
}
```

Note: `LogEntry` already derives `Serialize`, so `serde_json::json!` can serialize it directly.

---

## Fix 4: Proxy Log Collection Pipeline

**Problem**: `proxy.rs` does not collect structured logs, record latency, count RPM, or broadcast events via WebSocket. The data infrastructure (log_buffer, rpm_counter, ws_broadcaster, provider.record_latency) exists but is not connected.

**File**: `src/proxy.rs`

### 4.1 Add model extraction helper

Add at the bottom of `proxy.rs`:

```rust
/// Best-effort extract "model" field from request body JSON (check first 2KB).
fn extract_model(body: &[u8]) -> String {
    let check_len = body.len().min(2048);
    if let Ok(s) = std::str::from_utf8(&body[..check_len]) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
                return m.to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Generate a short random request ID.
fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let id: String = (0..12)
        .map(|_| {
            let idx = rng.random_range(0..36u8);
            if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
        })
        .collect();
    format!("req_{}", id)
}
```

### 4.2 Modify `proxy_handler` to collect logs

In `proxy_handler()`, add tracking variables at the top (after existing variable declarations):

```rust
// ADD after line 41 (let mut last_error ...):
let request_id = generate_request_id();
let proxy_start = std::time::Instant::now();
let request_bytes = body_bytes.len() as u64;
let model = extract_model(&body_bytes);
let mut failover_chain: Vec<String> = Vec::new();
let mut final_provider = String::new();
let mut final_status: u16 = 503;
let mut response_bytes: u64 = 0;
let mut ttfb_ms: u64 = 0;
```

Inside the retry loop, after `attempt_request` returns, record the result:

```rust
// Modify the match block in the retry loop (~line 75-96):
match attempt_request(...).await {
    Ok((response, provider_response_bytes, provider_ttfb_ms)) => {
        let status_code = response.status().as_u16();
        failover_chain.push(format!("{}:{}", provider_name, status_code));
        final_provider = provider_name.clone();
        final_status = status_code;
        response_bytes = provider_response_bytes;
        ttfb_ms = provider_ttfb_ms;

        // Record latency
        let latency_ms = proxy_start.elapsed().as_millis() as u64;
        provider.record_latency(latency_ms);

        // Build and store log entry
        let log_entry = crate::stats::LogEntry {
            id: request_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            method: method.to_string(),
            path: path.clone(),
            provider: final_provider.clone(),
            status: final_status,
            duration_ms: latency_ms,
            ttfb_ms,
            model: model.clone(),
            streaming: is_sse,
            request_bytes,
            response_bytes,
            failover_chain: if failover_chain.len() > 1 {
                Some(failover_chain.clone())
            } else {
                None
            },
        };
        state.log_buffer.push(log_entry.clone());
        state.rpm_counter.increment();
        state.ws_broadcaster.broadcast_log(log_entry);

        return Ok(response);
    }
    Err(e) => {
        failover_chain.push(format!("{}:err", provider_name));
        // ... existing error handling ...
    }
}
```

### 4.3 Modify `attempt_request` return type

Change `attempt_request` to also return response_bytes and ttfb_ms:

```rust
// BEFORE:
async fn attempt_request(...) -> Result<hyper::Response<ResponseBody>, crate::error::ProxyError>

// AFTER:
async fn attempt_request(...) -> Result<(hyper::Response<ResponseBody>, u64, u64), crate::error::ProxyError>
// Returns: (response, response_bytes, ttfb_ms)
```

In `attempt_request`, capture TTFB (time to first byte = time until response headers received):

```rust
let start = std::time::Instant::now();
let response = tokio::time::timeout(timeout, client.request(upstream_req))
    .await
    .map_err(|_| ...)?
    .map_err(|e| ...)?;

let ttfb_ms = start.elapsed().as_millis() as u64;
```

For the success path, return tuple:

```rust
// Non-SSE success:
let response_bytes = response_bytes_data.len() as u64;
Ok((downstream_response.body(full(response_bytes_data)).unwrap(), response_bytes, ttfb_ms))

// SSE success:
Ok((downstream_response.body(body).unwrap(), 0, ttfb_ms))  // response_bytes unknown for streaming
```

### 4.4 Also log failed requests (after retry loop)

After the retry loop exits (all providers failed), also push a log entry:

```rust
// After line 106 (the "All providers failed" response):
let log_entry = crate::stats::LogEntry {
    id: request_id,
    timestamp: chrono::Utc::now().to_rfc3339(),
    method: method.to_string(),
    path: path.clone(),
    provider: final_provider,
    status: 503,
    duration_ms: proxy_start.elapsed().as_millis() as u64,
    ttfb_ms: 0,
    model,
    streaming: is_sse,
    request_bytes,
    response_bytes: 0,
    failover_chain: if failover_chain.len() > 1 { Some(failover_chain) } else { None },
};
state.log_buffer.push(log_entry.clone());
state.rpm_counter.increment();
state.ws_broadcaster.broadcast_log(log_entry);
```

---

## Fix 5: WebSocket Upgrade + Main Routing

### 5.1 Implement real WebSocket upgrade handler

**File**: `src/ws.rs`

Replace `handle_ws_upgrade` with a real tokio-tungstenite implementation:

```rust
use tokio_tungstenite::tungstenite::protocol::Message;
use futures_util::{StreamExt, SinkExt};

/// Handle WebSocket upgrade from HTTP request.
/// Uses hyper's upgrade mechanism + tokio-tungstenite.
pub async fn handle_ws_connection(
    upgraded: hyper::upgrade::Upgraded,
    broadcaster: std::sync::Arc<WsBroadcaster>,
) {
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        hyper_util::rt::TokioIo::new(upgraded),
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    ).await;

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = broadcaster.subscribe();

    // Spawn a task to forward broadcast messages to the WS client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Read from client (handle subscribe messages, ping/pong)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(data) => {
                    // Pong is handled automatically by tungstenite
                    let _ = data;
                }
                Message::Text(_text) => {
                    // Could handle subscribe filter here
                    // For now, all events are sent to all clients
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// Check if request is a WebSocket upgrade and perform the upgrade.
/// Returns a 101 Switching Protocols response if upgrade is valid.
pub fn ws_upgrade_response(
    req: &hyper::Request<hyper::body::Incoming>,
) -> Option<hyper::Response<http_body_util::Full<hyper::body::Bytes>>> {
    use http_body_util::Full;
    use hyper::body::Bytes;

    // Verify this is a WebSocket upgrade request
    let upgrade = req.headers().get(hyper::header::UPGRADE)?;
    if upgrade.to_str().ok()? != "websocket" {
        return None;
    }

    let key = req.headers().get("sec-websocket-key")?;
    let accept = derive_accept_key(key.as_bytes());

    let resp = hyper::Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "Upgrade")
        .header("Sec-WebSocket-Accept", accept)
        .body(Full::new(Bytes::new()))
        .ok()?;

    Some(resp)
}

/// Derive the Sec-WebSocket-Accept value from the client key.
fn derive_accept_key(key: &[u8]) -> String {
    use std::io::Write;
    // The WebSocket GUID as defined in RFC 6455
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = sha1_smol::Sha1::new();
    hasher.update(key);
    hasher.update(WS_GUID);
    base64_encode(&hasher.digest().bytes())
}
```

> **Note**: The above `derive_accept_key` requires SHA-1 hashing and base64 encoding. Two approaches:
>
> **Option A (Recommended)**: Let `tokio-tungstenite` handle the upgrade entirely via `hyper::upgrade::on()`:

```rust
/// Simpler approach: let hyper handle upgrade, then wrap with tungstenite
pub async fn handle_ws_request(
    req: hyper::Request<hyper::body::Incoming>,
    state: crate::proxy::AppState,
) -> hyper::Response<http_body_util::Full<hyper::body::Bytes>> {
    use http_body_util::Full;
    use hyper::body::Bytes;

    // Spawn the WebSocket handler after upgrade completes
    let broadcaster = state.ws_broadcaster.clone();
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                handle_ws_connection(upgraded, broadcaster).await;
            }
            Err(e) => {
                tracing::error!(error = ?e, "WebSocket upgrade failed");
            }
        }
    });

    // Return 101 Switching Protocols
    // Note: hyper automatically sends upgrade headers when we call hyper::upgrade::on()
    hyper::Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "Upgrade")
        .body(Full::new(Bytes::new()))
        .unwrap()
}
```

> **Option B**: Use `tokio-tungstenite`'s `accept_hdr_async()` which handles the full handshake. This is simpler but requires the raw TCP stream (before hyper processes it).

**Recommended approach**: Use `hyper::upgrade::on()` (Option A) as it integrates cleanly with the existing hyper server setup.

### 5.2 Add `/zz/ws` route to main.rs

**File**: `src/main.rs`

In the `service_fn` closure (~line 116), add WebSocket routing **before** the `/zz/api/` check:

```rust
// ADD before line 119:
// Handle WebSocket upgrade (/zz/ws)
if path == "/zz/ws" {
    let resp = ws::handle_ws_request(req, state).await;
    return Ok::<_, hyper::Error>(resp.map(|b| b.map_err(|never| match never {}).boxed()));
}
```

### 5.3 Add periodic stats broadcast task

**File**: `src/main.rs`

After server setup, before the `loop` (~line 103), spawn a background task:

```rust
// Spawn periodic stats broadcaster (every 5 seconds)
{
    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let (total_requests, total_errors) = state_clone.provider_manager.get_total_stats();
            let all_stats = state_clone.provider_manager.get_all_stats();
            let active = all_stats.iter().filter(|s| s.enabled).count();
            let healthy = all_stats.iter().filter(|s| s.state == "healthy" && s.enabled).count();
            let total = all_stats.len();
            let config = state_clone.config.read().unwrap();

            let snapshot = crate::ws::StatsSnapshot {
                total_requests,
                total_errors,
                requests_per_minute: state_clone.rpm_counter.get_rpm(),
                active_providers: active,
                healthy_providers: healthy,
                total_providers: total,
                uptime_secs: state_clone.start_time.elapsed().as_secs(),
            };
            state_clone.ws_broadcaster.broadcast_stats(snapshot);
        }
    });
}
```

### 5.4 Fix legacy admin endpoints

**File**: `src/main.rs`

Replace the commented-out legacy handler (~lines 135-138) with actual routing:

```rust
// Handle legacy admin endpoints
if path == "/zz/health" || path == "/zz/stats" || path == "/zz/reload" {
    if let Some(resp) = admin::handle_admin_request(&path, &state) {
        return Ok::<_, hyper::Error>(resp);
    }
}
```

> This requires `admin.rs` to accept `AppState` properly. If the current `admin::handle_admin_request` signature doesn't match, adapt it or inline the 3 handlers.

### 5.5 Broadcast provider state changes

**File**: `src/provider.rs`

The `ProviderManager` methods that change state should broadcast via WebSocket. This requires passing `ws_broadcaster` to `ProviderManager`, or doing it at the call site.

**Approach**: Broadcast at the call site (in `proxy.rs` and `admin_api.rs`).

In `admin_api.rs`, after `enable_provider` / `disable_provider` / `reset_provider`:

```rust
// In handle_enable_provider, after provider.set_enabled(true):
state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);

// In handle_disable_provider, after provider.set_enabled(false):
state.ws_broadcaster.broadcast_provider_state(name, "disabled", None);

// In handle_reset_provider, after provider.reset():
state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);
```

In `proxy.rs`, after `mark_quota_exhausted` and `mark_failure`:

```rust
// After state.provider_manager.mark_quota_exhausted(provider_name):
let cooldown_secs = state.config.read().unwrap().health.cooldown_secs;
let cooldown_until = (chrono::Utc::now() + chrono::Duration::seconds(cooldown_secs as i64)).to_rfc3339();
state.ws_broadcaster.broadcast_provider_state(
    provider_name, "cooldown", Some(cooldown_until)
);

// After state.provider_manager.mark_failure(provider_name):
// Check if the provider is now unhealthy
if let Some(p) = state.provider_manager.get_by_name(provider_name) {
    let stats = p.get_stats();
    if stats.state == "unhealthy" {
        state.ws_broadcaster.broadcast_provider_state(provider_name, "unhealthy", None);
    }
}
```

---

## Fix 6: ProviderManager Add/Delete/Update

**File**: `src/provider.rs`

### 6.1 Add `add_provider` method

```rust
impl ProviderManager {
    /// Add a new provider at runtime.
    pub fn add_provider(&self, config: crate::config::ProviderConfig) -> Result<(), String> {
        if self.providers.contains_key(&config.name) {
            return Err(format!("Provider already exists: {}", config.name));
        }
        let provider = Arc::new(Provider::new(config.clone()));
        self.providers.insert(config.name.clone(), provider);
        tracing::info!(provider = %config.name, "Added new provider at runtime");
        Ok(())
    }
}
```

### 6.2 Add `remove_provider` method

```rust
impl ProviderManager {
    /// Remove a provider at runtime.
    pub fn remove_provider(&self, name: &str) -> Result<(), String> {
        if self.providers.remove(name).is_none() {
            return Err(format!("Provider not found: {}", name));
        }
        tracing::info!(provider = %name, "Removed provider at runtime");
        Ok(())
    }
}
```

### 6.3 Add `update_provider_config` method

```rust
impl ProviderManager {
    /// Update a provider's configuration fields at runtime.
    /// Preserves runtime state (request counts, health state).
    pub fn update_provider_config(
        &self,
        name: &str,
        updates: ProviderConfigUpdate,
    ) -> Result<(), String> {
        let provider = self.providers.get(name)
            .ok_or_else(|| format!("Provider not found: {}", name))?;

        // We need to create a new Provider with updated config but preserve stats
        // Since config fields are in the Provider struct, we need careful update
        let mut new_config = provider.config.clone();
        if let Some(base_url) = updates.base_url { new_config.base_url = base_url; }
        if let Some(api_key) = updates.api_key { new_config.api_key = api_key; }
        if let Some(priority) = updates.priority { new_config.priority = priority; }
        if let Some(weight) = updates.weight { new_config.weight = weight; }
        if let Some(models) = updates.models { new_config.models = models; }
        if let Some(headers) = updates.headers { new_config.headers = headers; }
        if let Some(token_budget) = updates.token_budget { new_config.token_budget = token_budget; }

        drop(provider); // Release DashMap read lock

        // Replace with new provider (preserving name, resetting stats)
        // Note: This resets runtime stats. For a non-destructive update,
        // Provider.config would need interior mutability (Mutex/RwLock).
        let new_provider = Arc::new(Provider::new(new_config));
        self.providers.insert(name.to_string(), new_provider);

        tracing::info!(provider = %name, "Updated provider config at runtime");
        Ok(())
    }
}

/// Partial update for provider configuration.
pub struct ProviderConfigUpdate {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub priority: Option<usize>,
    pub weight: Option<usize>,
    pub models: Option<Vec<String>>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub token_budget: Option<Option<u64>>,
}
```

### 6.4 Wire into admin_api.rs handlers

**`handle_add_provider`** (~line 310):

Replace the placeholder with:

```rust
// After validation, replace the placeholder section:
match state.provider_manager.add_provider(new_config.clone()) {
    Ok(()) => {
        // Broadcast new provider via WebSocket
        state.ws_broadcaster.broadcast_provider_state(&new_config.name, "healthy", None);

        // Return created provider
        let mut resp = json_response(&serde_json::json!({
            "name": new_config.name,
            // ... (existing JSON)
        }));
        *resp.status_mut() = hyper::StatusCode::CREATED;
        resp
    }
    Err(e) => bad_request_response(&e),
}
```

**`handle_delete_provider`** (~line 379):

```rust
match state.provider_manager.remove_provider(name) {
    Ok(()) => json_response(&serde_json::json!({ "removed": name })),
    Err(e) => not_found_response(&e),
}
```

**`handle_update_provider`** (~line 337):

```rust
// After parsing UpdateProviderRequest:
// Handle enabled flag separately (doesn't require config update)
if let Some(enabled) = update_req.enabled {
    let provider = state.provider_manager.get_by_name(name).unwrap();
    provider.set_enabled(enabled);
    let status = if enabled { "healthy" } else { "disabled" };
    state.ws_broadcaster.broadcast_provider_state(name, status, None);
}

// Handle config fields update
let has_config_update = update_req.base_url.is_some()
    || update_req.api_key.is_some()
    || update_req.priority.is_some()
    || update_req.weight.is_some()
    || update_req.models.is_some()
    || update_req.headers.is_some();

if has_config_update {
    let update = crate::provider::ProviderConfigUpdate {
        base_url: update_req.base_url,
        api_key: update_req.api_key,
        priority: update_req.priority,
        weight: update_req.weight,
        models: update_req.models,
        headers: update_req.headers,
        token_budget: None,
    };
    if let Err(e) = state.provider_manager.update_provider_config(name, update) {
        return bad_request_response(&e);
    }
}

// Return updated provider
handle_get_provider(name, state).await
```

---

## Fix 7: Provider Cooldown Until Time

**Problem**: `handle_list_providers` and `handle_get_provider` always return `cooldown_until: null` even when provider is in cooldown state.

**File**: `src/provider.rs`

Add a method to get cooldown_until:

```rust
impl Provider {
    /// Get the cooldown expiry time, if provider is in cooldown state.
    pub fn get_cooldown_until(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        match &*state {
            ProviderState::Cooldown { until } => {
                if chrono::Utc::now() < *until {
                    Some(until.to_rfc3339())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
```

Update `get_stats()` to include cooldown_until:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStats {
    // ... existing fields ...
    pub cooldown_until: Option<String>,  // ADD
}

// In get_stats():
ProviderStats {
    // ... existing ...
    cooldown_until: self.get_cooldown_until(),  // ADD
}
```

Then in `admin_api.rs`, use `stats.cooldown_until` instead of `serde_json::Value::Null`:

```rust
"cooldown_until": stats.cooldown_until,
```

---

## Fix 8: Test Provider — Real HTTP Request

**Problem**: `handle_test_provider` returns hardcoded `{ success: true, latency_ms: 150 }`.

**File**: `src/admin_api.rs`

Replace with a real HTTP request:

```rust
async fn handle_test_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name)),
    };

    let test_url = format!("{}/v1/models", provider.config.base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();

    // Build HTTPS client
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_only()
        .enable_http1()
        .build();

    let client: hyper_util::client::legacy::Client<_, http_body_util::Full<Bytes>> =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build(https);

    let req = hyper::Request::builder()
        .method(hyper::Method::GET)
        .uri(&test_url)
        .header(hyper::header::AUTHORIZATION, format!("Bearer {}", provider.config.api_key))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.request(req)
    ).await {
        Ok(Ok(resp)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let status_code = resp.status().as_u16();
            let success = resp.status().is_success();
            json_response(&serde_json::json!({
                "success": success,
                "latency_ms": latency_ms,
                "status_code": status_code
            }))
        }
        Ok(Err(e)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            json_response(&serde_json::json!({
                "success": false,
                "latency_ms": latency_ms,
                "status_code": 0,
                "error": e.to_string()
            }))
        }
        Err(_) => {
            json_response(&serde_json::json!({
                "success": false,
                "latency_ms": 10000,
                "status_code": 0,
                "error": "Connection timeout"
            }))
        }
    }
}
```

---

## Additional Dependencies

If using `hyper::upgrade::on()` for WebSocket, no additional dependencies are needed beyond existing `tokio-tungstenite`.

If SHA-1 is needed for manual WebSocket handshake:
```toml
sha1_smol = "1.0"
base64 = "0.22"
```

---

## Verification Checklist

After all fixes are applied:

```bash
# 1. Build
cargo build

# 2. Run backend
cargo run -- --config config.toml.example

# 3. Test API endpoints
curl http://127.0.0.1:9090/zz/api/health | jq
curl http://127.0.0.1:9090/zz/api/providers | jq
curl http://127.0.0.1:9090/zz/api/stats | jq
curl http://127.0.0.1:9090/zz/api/routing | jq
curl http://127.0.0.1:9090/zz/api/routing/rules | jq
curl http://127.0.0.1:9090/zz/api/config | jq

# 4. Test WebSocket
websocat ws://127.0.0.1:9090/zz/ws
# Should receive stats snapshots every 5 seconds

# 5. Test CORS preflight
curl -X OPTIONS http://127.0.0.1:9090/zz/api/providers -i
# Should return 204 with CORS headers

# 6. Test proxy + log collection
curl -X POST http://127.0.0.1:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen-plus","messages":[{"role":"user","content":"hi"}]}'
# Then check logs:
curl http://127.0.0.1:9090/zz/api/logs | jq
# Should have 1 entry

# 7. Start frontend
cd ui && pnpm dev
# Open http://localhost:5173
# Verify: Overview stats, Providers list, Logs streaming, Config editor

# 8. End-to-end: send proxy request and watch UI update in real time
```

---

## File Change Summary

| File | Changes | Lines (~) |
|------|---------|-----------|
| `src/admin_api.rs` | Fix rules path, real log_buffer/rpm/uptime, wire add/delete/update | ~80 |
| `src/proxy.rs` | Add log collection, model extraction, request ID, latency recording | ~60 |
| `src/ws.rs` | Real WebSocket upgrade handler | ~80 |
| `src/main.rs` | Add /zz/ws route, stats broadcast task, fix legacy endpoints | ~30 |
| `src/provider.rs` | Add add/remove/update methods, cooldown_until, ProviderConfigUpdate | ~60 |
| `Cargo.toml` | Possibly add sha1_smol + base64 (if manual WS handshake) | ~2 |
| **Total** | | **~312 lines** |
