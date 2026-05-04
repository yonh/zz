//! Admin API module - REST endpoints for dashboard
//!
//! All endpoints are prefixed with /zz/api/

use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::{Bytes, Incoming};

type ResponseBody = BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> ResponseBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn get_cors_config(state: &crate::proxy::AppState) -> Vec<String> {
    let config = state.config.read().unwrap();
    config.admin.allowed_origins.clone()
}

fn json_response<T: serde::Serialize>(data: &T, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let body = serde_json::to_string(data).unwrap_or_else(|_| "{\"error\":\" serialization failed\"}".to_string());
    let mut resp = hyper::Response::new(full(body));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    let allowed_origins = get_cors_config(state);
    crate::cors::add_cors_headers(&mut resp, &allowed_origins, origin);
    resp
}

/// Mask API key for safe display (show first 4 and last 4 chars)
fn mask_api_key(key: &str) -> String {
    if key.len() <= 12 {
        "*".repeat(key.len().min(8))
    } else {
        format!("{}****{}", &key[..4], &key[key.len()-4..])
    }
}

fn error_response(message: &str, status: hyper::StatusCode, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let body = serde_json::json!({ "error": message });
    let mut resp = hyper::Response::new(full(serde_json::to_string(&body).unwrap()));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    *resp.status_mut() = status;
    let allowed_origins = get_cors_config(state);
    crate::cors::add_cors_headers(&mut resp, &allowed_origins, origin);
    resp
}

fn not_found_response(message: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    error_response(message, hyper::StatusCode::NOT_FOUND, state, origin)
}

fn bad_request_response(message: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    error_response(message, hyper::StatusCode::BAD_REQUEST, state, origin)
}

fn unauthorized_response(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    error_response("Unauthorized: valid API key required", hyper::StatusCode::UNAUTHORIZED, state, origin)
}

/// Parse query parameters from URI
fn parse_query_params(uri: &hyper::Uri) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    if let Some(query) = uri.query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }
    }
    params
}

/// Extract provider name from path like /zz/api/providers/{name}/...
fn extract_provider_name(path: &str, prefix: &str) -> Option<String> {
    let rest = path.strip_prefix(prefix)?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn extract_request_id(path: &str, prefix: &str) -> Option<String> {
    let rest = path.strip_prefix(prefix)?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Check if the request is authenticated for admin API access.
///
/// Returns true if:
/// - Admin auth is disabled (admin.enabled = false, the default)
/// - Admin api_key is empty
/// - Request carries a valid `Authorization: Bearer <key>` header
/// - Request carries a valid `X-Admin-Key: <key>` header
fn check_auth(req: &hyper::Request<Incoming>, state: &crate::proxy::AppState) -> bool {
    let config = state.config.read().unwrap();
    if !config.admin.enabled {
        return true;
    }
    if config.admin.api_key.is_empty() {
        return true;
    }

    let expected = &config.admin.api_key;

    // Check Authorization: Bearer <key>
    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if constant_time_eq(token.as_bytes(), expected.as_bytes()) {
                    return true;
                }
            }
        }
    }

    // Check X-Admin-Key header
    if let Some(key) = req.headers().get("x-admin-key") {
        if let Ok(key_str) = key.to_str() {
            if constant_time_eq(key_str.as_bytes(), expected.as_bytes()) {
                return true;
            }
        }
    }

    false
}

/// Constant-time byte comparison to prevent timing attacks on auth keys.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Endpoints that are always public (no auth required).
fn is_public_endpoint(path: &str) -> bool {
    path == "/zz/api/health" || path == "/zz/api/version"
}

/// Main entry point for API request handling
pub async fn handle_api_request(
    req: hyper::Request<Incoming>,
    state: crate::proxy::AppState,
) -> Option<hyper::Response<ResponseBody>> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Extract Origin header for CORS
    let origin = req.headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let allowed_origins = get_cors_config(&state);

    // Handle OPTIONS preflight
    if method == hyper::Method::OPTIONS {
        let resp = crate::cors::preflight_response(&allowed_origins, origin.as_deref())
            .map(|b| b.map_err(|never| match never {}).boxed());
        return Some(resp);
    }

    let origin_ref = origin.as_deref();

    // Auth check: skip for public endpoints
    if !is_public_endpoint(path) && !check_auth(&req, &state) {
        return Some(unauthorized_response(&state, origin_ref));
    }

    // Only handle /zz/api/* paths
    if !path.starts_with("/zz/api/") {
        return None;
    }

    // Route to handlers
    let response = match (path, &method) {
        // Provider endpoints
        ("/zz/api/providers", &hyper::Method::GET) => handle_list_providers(&state, origin_ref).await,
        ("/zz/api/providers", &hyper::Method::POST) => {
            handle_add_provider(req, &state, origin_ref).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/test") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_test_provider(&name, &state, origin_ref).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/enable") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_enable_provider(&name, &state, origin_ref).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/disable") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_disable_provider(&name, &state, origin_ref).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/reset") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_reset_provider(&name, &state, origin_ref).await
        }
        (p, &hyper::Method::GET) if p.starts_with("/zz/api/providers/") && !p.contains("/test") && !p.contains("/enable") && !p.contains("/disable") && !p.contains("/reset") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_get_provider(&name, &state, origin_ref).await
        }
        (p, &hyper::Method::PUT) if p.starts_with("/zz/api/providers/") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_update_provider(req, &name, &state, origin_ref).await
        }
        (p, &hyper::Method::DELETE) if p.starts_with("/zz/api/providers/") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_delete_provider(&name, &state, origin_ref).await
        }

        // Routing endpoints
        ("/zz/api/routing", &hyper::Method::GET) => handle_get_routing(&state, origin_ref).await,
        ("/zz/api/routing", &hyper::Method::PUT) => {
            handle_update_routing(req, &state, origin_ref).await
        }
        ("/zz/api/routing/rules", &hyper::Method::GET) => handle_get_rules(&state, origin_ref).await,
        ("/zz/api/routing/rules", &hyper::Method::PUT) => {
            handle_update_rules(req, &state, origin_ref).await
        }

        // Model pins endpoints
        ("/zz/api/routing/pins", &hyper::Method::GET) => handle_get_pins(&state, origin_ref).await,
        ("/zz/api/routing/pins", &hyper::Method::PUT) => {
            handle_update_pins(req, &state, origin_ref).await
        }
        (p, &hyper::Method::DELETE) if p.starts_with("/zz/api/routing/pins/") => {
            handle_delete_pin(req, &state, origin_ref).await
        }

        // Stats endpoints
        ("/zz/api/stats", &hyper::Method::GET) => handle_get_stats(&state, origin_ref).await,
        ("/zz/api/stats/timeseries", &hyper::Method::GET) => {
            handle_get_timeseries(req.uri(), &state, origin_ref).await
        }
        ("/zz/api/logs", &hyper::Method::GET) => {
            handle_get_logs(req.uri(), &state, origin_ref).await
        }

        ("/zz/api/request-journal", &hyper::Method::GET) => {
            handle_list_request_journal(req.uri(), &state, origin_ref).await
        }
        ("/zz/api/request-journal/status", &hyper::Method::GET) => {
            handle_request_journal_status(&state, origin_ref).await
        }
        ("/zz/api/request-journal/facets", &hyper::Method::GET) => {
            handle_request_journal_facets(&state, origin_ref).await
        }
        ("/zz/api/request-journal/export", &hyper::Method::GET) => {
            handle_export_request_journal(req.uri(), &state, origin_ref).await
        }
        (p, &hyper::Method::GET) if p.starts_with("/zz/api/request-journal/") && !p.contains("/export") && !p.contains("/status") => {
            let id = extract_request_id(p, "/zz/api/request-journal/")?;
            handle_get_request_journal_entry(&id, req.uri(), &state, origin_ref).await
        }

        // Config endpoints
        ("/zz/api/config", &hyper::Method::GET) => handle_get_config(&state, origin_ref).await,
        ("/zz/api/config", &hyper::Method::PUT) => {
            handle_update_config(req, &state, origin_ref).await
        }
        ("/zz/api/config/validate", &hyper::Method::POST) => {
            handle_validate_config(req, &state, origin_ref).await
        }

        // System endpoints
        ("/zz/api/health", &hyper::Method::GET) => handle_health(&state, origin_ref).await,
        ("/zz/api/version", &hyper::Method::GET) => handle_version(&state, origin_ref).await,

        _ => return Some(not_found_response("Endpoint not found", &state, origin_ref)),
    };

    Some(response)
}

// ============================================================================
// Provider Handlers
// ============================================================================

async fn handle_list_providers(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let providers = state.provider_manager.get_all_stats();
    let config = state.config.read().unwrap();

    let provider_list: Vec<serde_json::Value> = providers.iter().map(|stats| {
        let provider_config = config.provider_configs.iter()
            .find(|p| p.name == stats.name);

        let error_rate = if stats.request_count > 0 {
            (stats.error_count as f64 / stats.request_count as f64) * 100.0
        } else {
            0.0
        };

        let masked_key = provider_config
            .map(|p| mask_api_key(&p.api_key))
            .unwrap_or_default();

        serde_json::json!({
            "name": stats.name,
            "base_url": provider_config.map(|p| p.base_url.as_str()).unwrap_or(""),
            "api_key_masked": masked_key,
            "priority": provider_config.map(|p| p.priority).unwrap_or(0),
            "weight": provider_config.map(|p| p.weight).unwrap_or(0),
            "enabled": stats.enabled,
            "models": provider_config.map(|p| p.models.clone()).unwrap_or_default(),
            "headers": provider_config.map(|p| p.headers.clone()).unwrap_or_default(),
            "token_budget": serde_json::Value::Null,
            "status": if !stats.enabled { "disabled" } else { stats.state.as_str() },
            "cooldown_until": serde_json::Value::Null,
            "consecutive_failures": stats.failure_count,
            "stats": {
                "total_requests": stats.request_count,
                "total_errors": stats.error_count,
                "error_rate": (error_rate * 100.0).round() / 100.0,
                "avg_latency_ms": stats.avg_latency_ms,
                "latency_history": stats.latency_history,
                "prompt_tokens": stats.prompt_tokens,
                "completion_tokens": stats.completion_tokens,
                "total_tokens": stats.prompt_tokens + stats.completion_tokens
            }
        })
    }).collect();

    json_response(&serde_json::json!({ "providers": provider_list }), state, origin)
}

async fn handle_get_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let all_stats = state.provider_manager.get_all_stats();
    let stats = match all_stats.iter().find(|s| s.name == name) {
        Some(s) => s,
        None => return not_found_response(&format!("Provider not found: {}", name), state, origin),
    };

    let config = state.config.read().unwrap();
    let provider_config = config.provider_configs.iter()
        .find(|p| p.name == name);

    let error_rate = if stats.request_count > 0 {
        (stats.error_count as f64 / stats.request_count as f64) * 100.0
    } else {
        0.0
    };

    let provider = serde_json::json!({
        "name": stats.name,
        "base_url": provider_config.map(|p| p.base_url.as_str()).unwrap_or(""),
        "api_key_masked": provider_config.map(|p| mask_api_key(&p.api_key)).unwrap_or_default(),
        "priority": provider_config.map(|p| p.priority).unwrap_or(0),
        "weight": provider_config.map(|p| p.weight).unwrap_or(0),
        "enabled": stats.enabled,
        "models": provider_config.map(|p| p.models.clone()).unwrap_or_default(),
        "headers": provider_config.map(|p| p.headers.clone()).unwrap_or_default(),
        "token_budget": serde_json::Value::Null,
        "status": if !stats.enabled { "disabled" } else { stats.state.as_str() },
        "cooldown_until": serde_json::Value::Null,
        "consecutive_failures": stats.failure_count,
        "stats": {
            "total_requests": stats.request_count,
            "total_errors": stats.error_count,
            "error_rate": (error_rate * 100.0).round() / 100.0,
            "avg_latency_ms": stats.avg_latency_ms,
            "latency_history": stats.latency_history,
            "prompt_tokens": stats.prompt_tokens,
            "completion_tokens": stats.completion_tokens,
            "total_tokens": stats.prompt_tokens + stats.completion_tokens
        }
    });

    json_response(&provider, state, origin)
}

async fn handle_add_provider(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    #[derive(serde::Deserialize)]
    struct AddProviderRequest {
        name: String,
        base_url: String,
        api_key: String,
        #[serde(default)]
        priority: usize,
        #[serde(default)]
        weight: usize,
        #[serde(default)]
        models: Vec<String>,
        #[serde(default)]
        headers: std::collections::HashMap<String, String>,
    }

    let add_req: AddProviderRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    if add_req.name.is_empty() {
        return bad_request_response("name is required", state, origin);
    }
    if add_req.base_url.is_empty() {
        return bad_request_response("base_url is required", state, origin);
    }
    if add_req.api_key.is_empty() {
        return bad_request_response("api_key is required", state, origin);
    }

    if state.provider_manager.get_by_name(&add_req.name).is_some() {
        return bad_request_response("Provider name already exists", state, origin);
    }

    let new_config = crate::config::ProviderConfig {
        name: add_req.name.clone(),
        base_url: add_req.base_url,
        api_key: add_req.api_key,
        priority: add_req.priority,
        weight: add_req.weight,
        models: add_req.models,
        headers: add_req.headers,
        token_budget: None,
        enabled: true,
    };

    match state.provider_manager.add_provider(new_config.clone()) {
        Ok(()) => {
            state.ws_broadcaster.broadcast_provider_state(&new_config.name, "healthy", None);

            let mut resp = json_response(&serde_json::json!({
                "name": new_config.name,
                "base_url": new_config.base_url,
                "api_key_masked": mask_api_key(&new_config.api_key),
                "priority": new_config.priority,
                "weight": new_config.weight,
                "enabled": true,
                "models": new_config.models,
                "headers": new_config.headers,
                "token_budget": serde_json::Value::Null,
                "status": "healthy",
                "cooldown_until": serde_json::Value::Null,
                "consecutive_failures": 0,
                "stats": {
                    "total_requests": 0,
                    "total_errors": 0,
                    "error_rate": 0.0,
                    "avg_latency_ms": 0,
                    "latency_history": [],
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0
                }
            }), state, origin);
            *resp.status_mut() = hyper::StatusCode::CREATED;
            resp
        }
        Err(e) => bad_request_response(&e, state, origin),
    }
}

async fn handle_update_provider(
    req: hyper::Request<Incoming>,
    name: &str,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name), state, origin);
    }

    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    #[derive(serde::Deserialize)]
    struct UpdateProviderRequest {
        base_url: Option<String>,
        api_key: Option<String>,
        priority: Option<usize>,
        weight: Option<usize>,
        enabled: Option<bool>,
        models: Option<Vec<String>>,
        headers: Option<std::collections::HashMap<String, String>>,
    }

    let update_req: UpdateProviderRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    if let Some(enabled) = update_req.enabled {
        let provider = state.provider_manager.get_by_name(name).unwrap();
        provider.set_enabled(enabled);
        let status = if enabled { "healthy" } else { "disabled" };
        state.ws_broadcaster.broadcast_provider_state(name, status, None);
    }

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
            return bad_request_response(&e, state, origin);
        }
    }

    handle_get_provider(name, state, origin).await
}

async fn handle_delete_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name), state, origin);
    }

    match state.provider_manager.remove_provider(name) {
        Ok(()) => json_response(&serde_json::json!({ "removed": name }), state, origin),
        Err(e) => not_found_response(&e, state, origin),
    }
}

async fn handle_test_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name), state, origin);
    }

    json_response(&serde_json::json!({
        "success": true,
        "latency_ms": 150,
        "status_code": 200
    }), state, origin)
}

async fn handle_enable_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name), state, origin),
    };

    provider.set_enabled(true);
    provider.reset();
    state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);

    json_response(&serde_json::json!({ "name": name, "enabled": true }), state, origin)
}

async fn handle_disable_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name), state, origin),
    };

    provider.set_enabled(false);
    state.ws_broadcaster.broadcast_provider_state(name, "disabled", None);

    json_response(&serde_json::json!({ "name": name, "enabled": false }), state, origin)
}

async fn handle_reset_provider(name: &str, state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name), state, origin),
    };

    provider.reset();
    state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);

    json_response(&serde_json::json!({ "name": name, "status": "healthy" }), state, origin)
}

// ============================================================================
// Routing Handlers
// ============================================================================

async fn handle_get_routing(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let config = state.config.read().unwrap();

    json_response(&serde_json::json!({
        "strategy": config.routing.strategy,
        "max_retries": config.routing.max_retries,
        "cooldown_secs": config.health.cooldown_secs,
        "failure_threshold": config.health.failure_threshold,
        "recovery_secs": config.health.recovery_secs,
        "pinned_provider": config.routing.pinned_provider
    }), state, origin)
}

async fn handle_update_routing(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    #[derive(serde::Deserialize)]
    struct UpdateRoutingRequest {
        strategy: Option<String>,
        max_retries: Option<usize>,
        cooldown_secs: Option<u64>,
        failure_threshold: Option<usize>,
        recovery_secs: Option<u64>,
        pinned_provider: Option<String>,
    }

    let update_req: UpdateRoutingRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    {
        let mut config = state.config.write().unwrap();
        if let Some(strategy) = update_req.strategy {
            config.routing.strategy = strategy;
        }
        if let Some(max_retries) = update_req.max_retries {
            config.routing.max_retries = max_retries;
        }
        if let Some(cooldown_secs) = update_req.cooldown_secs {
            config.health.cooldown_secs = cooldown_secs;
        }
        if let Some(failure_threshold) = update_req.failure_threshold {
            config.health.failure_threshold = failure_threshold;
        }
        if let Some(recovery_secs) = update_req.recovery_secs {
            config.health.recovery_secs = recovery_secs;
        }
        if update_req.pinned_provider.is_some() {
            config.routing.pinned_provider = update_req.pinned_provider;
        }
    }

    handle_get_routing(state, origin).await
}

async fn handle_get_rules(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let rules = state.model_rules.read().unwrap();
    let rules_json: Vec<serde_json::Value> = rules.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "pattern": r.pattern,
            "target_provider": r.target_provider
        })
    }).collect();
    json_response(&serde_json::json!({ "rules": rules_json }), state, origin)
}

async fn handle_update_rules(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
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
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    let new_rules: Vec<crate::router::ModelRule> = update_req.rules.into_iter()
        .enumerate()
        .map(|(i, r)| crate::router::ModelRule {
            id: format!("rule_{}", i + 1),
            pattern: r.pattern,
            target_provider: r.target_provider,
        })
        .collect();

    let rules_json: Vec<serde_json::Value> = new_rules.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "pattern": r.pattern,
            "target_provider": r.target_provider
        })
    }).collect();

    {
        let mut config = state.config.write().unwrap();
        config.routing.rules = new_rules.iter().map(|r| crate::config::ModelRuleConfig {
            pattern: r.pattern.clone(),
            target_provider: r.target_provider.clone(),
        }).collect();
    }

    *state.model_rules.write().unwrap() = new_rules;

    json_response(&serde_json::json!({ "rules": rules_json }), state, origin)
}

// ============================================================================
// Stats Handlers

// ============================================================================
// Model Pins Handlers
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct ModelPin {
    model: String,
    provider: String,
}

#[derive(Debug, serde::Deserialize)]
struct UpdatePinsRequest {
    pins: Vec<ModelPin>,
}

async fn handle_get_pins(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let pins: Vec<serde_json::Value> = state.model_pins
        .iter()
        .map(|entry| {
            serde_json::json!({
                "model": entry.key(),
                "provider": entry.value()
            })
        })
        .collect();

    json_response(&serde_json::json!({ "pins": pins }), state, origin)
}

async fn handle_update_pins(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    let update_req: UpdatePinsRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    for pin in &update_req.pins {
        if pin.model.is_empty() {
            return bad_request_response("Model name cannot be empty", state, origin);
        }
        if pin.provider.is_empty() {
            return bad_request_response("Provider name cannot be empty", state, origin);
        }
        if state.provider_manager.get_by_name(&pin.provider).is_none() {
            return bad_request_response(&format!("Provider not found: {}", pin.provider), state, origin);
        }
    }

    state.model_pins.clear();
    for pin in &update_req.pins {
        state.model_pins.insert(pin.model.clone(), pin.provider.clone());
        tracing::info!(
            model = %pin.model,
            provider = %pin.provider,
            "Model pinned to provider"
        );
    }

    let pins: Vec<serde_json::Value> = state.model_pins
        .iter()
        .map(|entry| {
            serde_json::json!({
                "model": entry.key(),
                "provider": entry.value()
            })
        })
        .collect();

    json_response(&serde_json::json!({ "pins": pins }), state, origin)
}

async fn handle_delete_pin(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let path = req.uri().path();

    let model = path
        .strip_prefix("/zz/api/routing/pins/")
        .map(|s| {
            percent_encoding::percent_decode_str(s)
                .decode_utf8_lossy()
                .to_string()
        });

    match model {
        Some(model) => {
            if state.model_pins.remove(&model).is_some() {
                tracing::info!(model = %model, "Model pin removed");
                json_response(&serde_json::json!({ "removed": model }), state, origin)
            } else {
                not_found_response(&format!("No pin found for model: {}", model), state, origin)
            }
        }
        None => bad_request_response("Invalid model in path", state, origin),
    }
}

// ============================================================================

async fn handle_get_stats(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let total_requests = state.provider_manager.get_total_stats().0;
    let all_stats = state.provider_manager.get_all_stats();

    let active_providers = all_stats.iter().filter(|s| s.enabled).count();
    let healthy_providers = all_stats.iter()
        .filter(|s| s.state == "healthy" && s.enabled)
        .count();
    let total_providers = all_stats.len();

    let (total_prompt_tokens, total_completion_tokens) = state.provider_manager.get_total_tokens();

    let config = state.config.read().unwrap();

    json_response(&serde_json::json!({
        "total_requests": total_requests,
        "requests_per_minute": state.rpm_counter.get_rpm(),
        "active_providers": active_providers,
        "healthy_providers": healthy_providers,
        "total_providers": total_providers,
        "strategy": config.routing.strategy,
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "tokens": {
            "prompt": total_prompt_tokens,
            "completion": total_completion_tokens,
            "total": total_prompt_tokens + total_completion_tokens
        }
    }), state, origin)
}

async fn handle_get_timeseries(
    _uri: &hyper::Uri,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let params = parse_query_params(_uri);
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("1h");

    json_response(&serde_json::json!({
        "period": period,
        "interval_secs": 60,
        "data": serde_json::Value::Array(vec![])
    }), state, origin)
}

async fn handle_get_logs(
    uri: &hyper::Uri,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
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
    }), state, origin)
}

// ============================================================================
// Config Handlers
// ============================================================================

async fn handle_get_config(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return error_response(&format!("Failed to read config: {}", e), hyper::StatusCode::INTERNAL_SERVER_ERROR, state, origin),
    };

    let metadata = match std::fs::metadata(&state.config_path) {
        Ok(m) => m,
        Err(_) => return error_response("Failed to read config metadata", hyper::StatusCode::INTERNAL_SERVER_ERROR, state, origin),
    };

    let last_modified = metadata.modified()
        .ok()
        .and_then(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            Some(datetime.to_rfc3339())
        })
        .unwrap_or_default();

    let last_reloaded = state.last_reloaded.lock().unwrap()
        .clone()
        .unwrap_or_else(|| last_modified.clone());

    json_response(&serde_json::json!({
        "content": content,
        "path": state.config_path,
        "last_modified": last_modified,
        "last_reloaded": last_reloaded
    }), state, origin)
}

async fn handle_update_config(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    #[derive(serde::Deserialize)]
    struct UpdateConfigRequest {
        content: String,
    }

    let update_req: UpdateConfigRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    match crate::config::Config::load_from_str(&update_req.content) {
        Ok(_) => {},
        Err(e) => {
            let body = serde_json::json!({
                "saved": false,
                "error": format!("TOML parse error: {}", e)
            });
            let mut resp = json_response(&body, state, origin);
            *resp.status_mut() = hyper::StatusCode::BAD_REQUEST;
            return resp;
        }
    }

    if let Err(e) = std::fs::write(&state.config_path, &update_req.content) {
        return error_response(&format!("Failed to write config: {}", e), hyper::StatusCode::INTERNAL_SERVER_ERROR, state, origin);
    }

    if let Err(e) = state.reload_config() {
        return json_response(&serde_json::json!({
            "saved": true,
            "reloaded": false,
            "error": e
        }), state, origin);
    }

    let now = chrono::Utc::now().to_rfc3339();

    json_response(&serde_json::json!({
        "saved": true,
        "reloaded": true,
        "last_modified": now,
        "last_reloaded": now
    }), state, origin)
}

async fn handle_validate_config(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body", state, origin),
    };

    #[derive(serde::Deserialize)]
    struct ValidateConfigRequest {
        content: String,
    }

    let validate_req: ValidateConfigRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e), state, origin),
    };

    match crate::config::Config::load_from_str(&validate_req.content) {
        Ok(_) => json_response(&serde_json::json!({ "valid": true }), state, origin),
        Err(e) => json_response(&serde_json::json!({
            "valid": false,
            "error": format!("{}", e)
        }), state, origin),
    }
}

// ============================================================================
// System Handlers
// ============================================================================

async fn handle_health(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    let providers = state.provider_manager.get_all_states();

    json_response(&serde_json::json!({
        "status": "ok",
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "providers": providers
    }), state, origin)
}

async fn handle_version(state: &crate::proxy::AppState, origin: Option<&str>) -> hyper::Response<ResponseBody> {
    json_response(&serde_json::json!({
        "version": "0.1.0",
        "build_time": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "rust_version": env!("CARGO_PKG_RUST_VERSION")
    }), state, origin)
}

// ============================================================================
// Request Journal Handlers
// ============================================================================

async fn handle_request_journal_status(
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let enabled = state.request_journal.is_enabled();
    let storage_dir = state.request_journal.storage_dir();

    let total_entries = if enabled {
        state.request_journal.total_count(&storage_dir).await.unwrap_or(0)
    } else {
        0
    };

    json_response(&serde_json::json!({
        "enabled": enabled,
        "storage_path": storage_dir,
        "total_entries": total_entries
    }), state, origin)
}

async fn handle_request_journal_facets(
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let enabled = state.request_journal.is_enabled();
    if !enabled {
        return json_response(&serde_json::json!({
            "clients": [],
            "providers": [],
            "models": []
        }), state, origin);
    }

    let storage_dir = state.request_journal.storage_dir();
    let (clients, providers, models) = state.request_journal.facets(&storage_dir).await;

    json_response(&serde_json::json!({
        "clients": clients,
        "providers": providers,
        "models": models
    }), state, origin)
}

async fn handle_list_request_journal(
    uri: &hyper::Uri,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let enabled = state.request_journal.is_enabled();

    if !enabled {
        return json_response(&serde_json::json!({
            "enabled": false,
            "entries": [],
            "total": 0,
            "offset": 0,
            "limit": 50,
            "message": "Request journal is disabled. Enable it in config.toml [observability.request_journal] enabled = true"
        }), state, origin);
    }

    let params = parse_query_params(uri);

    let offset: usize = params.get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let limit: usize = params.get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .min(200);

    let query = crate::request_journal::JournalQuery {
        client: params.get("client").cloned(),
        provider: params.get("provider").cloned(),
        model: params.get("model").cloned(),
        status: params.get("status").and_then(|v| v.parse().ok()),
        path: params.get("path").cloned(),
        date: params.get("date").cloned(),
        failed_only: params.get("failed").map(|v| v == "true").unwrap_or(false),
        slow_only: params.get("slow").map(|v| v == "true").unwrap_or(false),
    };

    let storage_dir = state.request_journal.storage_dir();

    match crate::request_journal::list_entries(&storage_dir, query, offset, limit, Some(&state.request_journal)).await {
        Ok((entries, total)) => {
            json_response(&serde_json::json!({
                "enabled": true,
                "entries": entries,
                "total": total,
                "offset": offset,
                "limit": limit
            }), state, origin)
        }
        Err(e) => error_response(&e, hyper::StatusCode::INTERNAL_SERVER_ERROR, state, origin),
    }
}

async fn handle_get_request_journal_entry(
    id: &str,
    uri: &hyper::Uri,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let storage_dir = state.request_journal.storage_dir();

    let params = parse_query_params(uri);
    let format = params.get("format").map(|v| v.as_str()).unwrap_or("json");

    if format == "summary" {
        match crate::request_journal::get_entry(&storage_dir, id).await {
            Ok(Some(entry)) => {
                let summary = crate::request_journal::format_timing_summary(&entry);
                let mut resp = hyper::Response::new(full(summary));
                resp.headers_mut().insert(
                    hyper::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8".parse().unwrap(),
                );
                let allowed_origins = get_cors_config(state);
                crate::cors::add_cors_headers(&mut resp, &allowed_origins, origin);
                return resp;
            }
            Ok(None) => return not_found_response(&format!("Request journal entry not found: {}", id), state, origin),
            Err(e) => return bad_request_response(&e, state, origin),
        }
    }

    match crate::request_journal::get_entry(&storage_dir, id).await {
        Ok(Some(entry)) => {
            let include_trace = params.get("include_trace").map(|v| v == "true").unwrap_or(false);

            if include_trace {
                let mut response = serde_json::to_value(&entry)
                    .unwrap_or_else(|e| {
                        tracing::error!(error = %e, "Failed to serialize journal entry");
                        serde_json::json!({"error": "serialization failed"})
                    });
                let trace_storage_dir = state.config.read().unwrap().observability.tracing.storage_dir.clone();
                if let Some(trace) = crate::trace_layer::get_trace(&trace_storage_dir, id).await {
                    if let Some(obj) = response.as_object_mut() {
                        obj.insert(
                            "trace".to_string(),
                            serde_json::to_value(&trace).unwrap_or(serde_json::Value::Null),
                        );
                    }
                }
                json_response(&response, state, origin)
            } else {
                json_response(&entry, state, origin)
            }
        }
        Ok(None) => not_found_response(&format!("Request journal entry not found: {}", id), state, origin),
        Err(e) => bad_request_response(&e, state, origin),
    }
}

async fn handle_export_request_journal(
    uri: &hyper::Uri,
    state: &crate::proxy::AppState,
    origin: Option<&str>,
) -> hyper::Response<ResponseBody> {
    let params = parse_query_params(uri);

    let query = crate::request_journal::JournalQuery {
        client: params.get("client").cloned(),
        provider: params.get("provider").cloned(),
        model: params.get("model").cloned(),
        status: params.get("status").and_then(|v| v.parse().ok()),
        path: params.get("path").cloned(),
        date: params.get("date").cloned(),
        failed_only: params.get("failed").map(|v| v == "true").unwrap_or(false),
        slow_only: params.get("slow").map(|v| v == "true").unwrap_or(false),
    };

    let storage_dir = state.request_journal.storage_dir();
    let format = params.get("format").map(|v| v.as_str()).unwrap_or("json");
    let limit: usize = params.get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000)
        .min(5000);

    match crate::request_journal::export_entries(&storage_dir, query, limit).await {
        Ok(entries) => {
            if format == "summary" {
                let mut summaries = Vec::with_capacity(entries.len());
                for entry in &entries {
                    summaries.push(crate::request_journal::format_timing_summary(entry));
                }
                let text = summaries.join("\n\n---\n\n");
                let mut resp = hyper::Response::new(full(text));
                resp.headers_mut().insert(
                    hyper::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8".parse().unwrap(),
                );
                resp.headers_mut().insert(
                    hyper::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"request-journal-summary.txt\"".parse().unwrap(),
                );
                let allowed_origins = get_cors_config(state);
                crate::cors::add_cors_headers(&mut resp, &allowed_origins, origin);
                resp
            } else {
                let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
                let mut resp = hyper::Response::new(full(json));
                resp.headers_mut().insert(
                    hyper::header::CONTENT_TYPE,
                    "application/json".parse().unwrap(),
                );
                resp.headers_mut().insert(
                    hyper::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"request-journal-export.json\"".parse().unwrap(),
                );
                let allowed_origins = get_cors_config(state);
                crate::cors::add_cors_headers(&mut resp, &allowed_origins, origin);
                resp
            }
        }
        Err(e) => error_response(&e, hyper::StatusCode::INTERNAL_SERVER_ERROR, state, origin),
    }
}
