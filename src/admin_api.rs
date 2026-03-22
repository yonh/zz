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

fn json_response<T: serde::Serialize>(data: &T) -> hyper::Response<ResponseBody> {
    let body = serde_json::to_string(data).unwrap_or_else(|_| "{\"error\":\" serialization failed\"}".to_string());
    let mut resp = hyper::Response::new(full(body));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    crate::cors::add_cors_headers(&mut resp);
    resp
}

fn error_response(message: &str, status: hyper::StatusCode) -> hyper::Response<ResponseBody> {
    let body = serde_json::json!({ "error": message });
    let mut resp = hyper::Response::new(full(serde_json::to_string(&body).unwrap()));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    *resp.status_mut() = status;
    crate::cors::add_cors_headers(&mut resp);
    resp
}

fn not_found_response(message: &str) -> hyper::Response<ResponseBody> {
    error_response(message, hyper::StatusCode::NOT_FOUND)
}

fn bad_request_response(message: &str) -> hyper::Response<ResponseBody> {
    error_response(message, hyper::StatusCode::BAD_REQUEST)
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
    // Get the next segment
    let end = rest.find('/').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Main entry point for API request handling
pub async fn handle_api_request(
    req: hyper::Request<Incoming>,
    state: crate::proxy::AppState,
) -> Option<hyper::Response<ResponseBody>> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Handle OPTIONS preflight
    if method == hyper::Method::OPTIONS {
        let mut resp = hyper::Response::new(full(""));
        crate::cors::add_cors_headers(&mut resp);
        *resp.status_mut() = hyper::StatusCode::NO_CONTENT;
        return Some(resp);
    }

    // Only handle /zz/api/* paths
    if !path.starts_with("/zz/api/") {
        return None;
    }

    // Route to handlers
    let response = match (path, &method) {
        // Provider endpoints
        ("/zz/api/providers", &hyper::Method::GET) => handle_list_providers(&state).await,
        ("/zz/api/providers", &hyper::Method::POST) => {
            handle_add_provider(req, &state).await
        }
        (p, &hyper::Method::GET) if p.starts_with("/zz/api/providers/") && p.ends_with("/test") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_test_provider(&name, &state).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/enable") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_enable_provider(&name, &state).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/disable") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_disable_provider(&name, &state).await
        }
        (p, &hyper::Method::POST) if p.starts_with("/zz/api/providers/") && p.ends_with("/reset") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_reset_provider(&name, &state).await
        }
        (p, &hyper::Method::GET) if p.starts_with("/zz/api/providers/") && !p.contains("/test") && !p.contains("/enable") && !p.contains("/disable") && !p.contains("/reset") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_get_provider(&name, &state).await
        }
        (p, &hyper::Method::PUT) if p.starts_with("/zz/api/providers/") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_update_provider(req, &name, &state).await
        }
        (p, &hyper::Method::DELETE) if p.starts_with("/zz/api/providers/") => {
            let name = extract_provider_name(p, "/zz/api/providers/")?;
            handle_delete_provider(&name, &state).await
        }

        // Routing endpoints
        ("/zz/api/routing", &hyper::Method::GET) => handle_get_routing(&state).await,
        ("/zz/api/routing", &hyper::Method::PUT) => {
            handle_update_routing(req, &state).await
        }
        ("/zz/api/routing/rules", &hyper::Method::GET) => handle_get_rules(&state).await,
        ("/zz/api/routing/rules", &hyper::Method::PUT) => {
            handle_update_rules(req, &state).await
        }

        // Stats endpoints
        ("/zz/api/stats", &hyper::Method::GET) => handle_get_stats(&state).await,
        ("/zz/api/stats/timeseries", &hyper::Method::GET) => {
            handle_get_timeseries(req.uri(), &state).await
        }
        ("/zz/api/logs", &hyper::Method::GET) => {
            handle_get_logs(req.uri(), &state).await
        }

        // Config endpoints
        ("/zz/api/config", &hyper::Method::GET) => handle_get_config(&state).await,
        ("/zz/api/config", &hyper::Method::PUT) => {
            handle_update_config(req, &state).await
        }
        ("/zz/api/config/validate", &hyper::Method::POST) => {
            handle_validate_config(req).await
        }

        // System endpoints
        ("/zz/api/health", &hyper::Method::GET) => handle_health(&state).await,
        ("/zz/api/version", &hyper::Method::GET) => handle_version().await,

        _ => return Some(not_found_response("Endpoint not found")),
    };

    Some(response)
}

// ============================================================================
// Provider Handlers
// ============================================================================

async fn handle_list_providers(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
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

        serde_json::json!({
            "name": stats.name,
            "base_url": provider_config.map(|p| p.base_url.as_str()).unwrap_or(""),
            "api_key": provider_config.map(|p| p.api_key.as_str()).unwrap_or(""),
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
                "latency_history": stats.latency_history
            }
        })
    }).collect();

    json_response(&serde_json::json!({ "providers": provider_list }))
}

async fn handle_get_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let all_stats = state.provider_manager.get_all_stats();
    let stats = match all_stats.iter().find(|s| s.name == name) {
        Some(s) => s,
        None => return not_found_response(&format!("Provider not found: {}", name)),
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
        "api_key": provider_config.map(|p| p.api_key.as_str()).unwrap_or(""),
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
            "latency_history": stats.latency_history
        }
    });

    json_response(&provider)
}

async fn handle_add_provider(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    // Collect body
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
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
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

    // Validate required fields
    if add_req.name.is_empty() {
        return bad_request_response("name is required");
    }
    if add_req.base_url.is_empty() {
        return bad_request_response("base_url is required");
    }
    if add_req.api_key.is_empty() {
        return bad_request_response("api_key is required");
    }

    // Check if provider already exists
    if state.provider_manager.get_by_name(&add_req.name).is_some() {
        return bad_request_response("Provider name already exists");
    }

    // Create new provider config
    let new_config = crate::config::ProviderConfig {
        name: add_req.name.clone(),
        base_url: add_req.base_url,
        api_key: add_req.api_key,
        priority: add_req.priority,
        weight: add_req.weight,
        models: add_req.models,
        headers: add_req.headers,
        token_budget: None,
    };

    // Add to provider manager
    match state.provider_manager.add_provider(new_config.clone()) {
        Ok(()) => {
            // Broadcast new provider via WebSocket
            state.ws_broadcaster.broadcast_provider_state(&new_config.name, "healthy", None);

            let mut resp = json_response(&serde_json::json!({
                "name": new_config.name,
                "base_url": new_config.base_url,
                "api_key": new_config.api_key,
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
                    "latency_history": []
                }
            }));
            *resp.status_mut() = hyper::StatusCode::CREATED;
            resp
        }
        Err(e) => bad_request_response(&e),
    }
}

async fn handle_update_provider(
    req: hyper::Request<Incoming>,
    name: &str,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    // Check provider exists
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name));
    }

    // Collect body
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
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
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

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
}

async fn handle_delete_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    // Check provider exists
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name));
    }

    // Remove provider
    match state.provider_manager.remove_provider(name) {
        Ok(()) => json_response(&serde_json::json!({ "removed": name })),
        Err(e) => not_found_response(&e),
    }
}

async fn handle_test_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    // Check provider exists
    if state.provider_manager.get_by_name(name).is_none() {
        return not_found_response(&format!("Provider not found: {}", name));
    }

    // Return mock test result
    // In real implementation, would make actual HTTP request to provider
    json_response(&serde_json::json!({
        "success": true,
        "latency_ms": 150,
        "status_code": 200
    }))
}

async fn handle_enable_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name)),
    };

    provider.set_enabled(true);
    provider.reset();

    // Broadcast state change
    state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);

    json_response(&serde_json::json!({ "name": name, "enabled": true }))
}

async fn handle_disable_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name)),
    };

    provider.set_enabled(false);

    // Broadcast state change
    state.ws_broadcaster.broadcast_provider_state(name, "disabled", None);

    json_response(&serde_json::json!({ "name": name, "enabled": false }))
}

async fn handle_reset_provider(name: &str, state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let provider = match state.provider_manager.get_by_name(name) {
        Some(p) => p,
        None => return not_found_response(&format!("Provider not found: {}", name)),
    };

    provider.reset();

    // Broadcast state change
    state.ws_broadcaster.broadcast_provider_state(name, "healthy", None);

    json_response(&serde_json::json!({ "name": name, "status": "healthy" }))
}

// ============================================================================
// Routing Handlers
// ============================================================================

async fn handle_get_routing(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let config = state.config.read().unwrap();

    json_response(&serde_json::json!({
        "strategy": config.routing.strategy,
        "max_retries": config.routing.max_retries,
        "cooldown_secs": config.health.cooldown_secs,
        "failure_threshold": config.health.failure_threshold,
        "recovery_secs": config.health.recovery_secs,
        "pinned_provider": config.routing.pinned_provider
    }))
}

async fn handle_update_routing(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    // Collect body
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
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
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

    // Update config
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

    handle_get_routing(state).await
}

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

async fn handle_update_rules(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    // Collect body
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

    // Build response JSON
    let rules_json: Vec<serde_json::Value> = new_rules.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "pattern": r.pattern,
            "target_provider": r.target_provider
        })
    }).collect();

    // Sync to config memory so GET /zz/api/config reflects current rules
    {
        let mut config = state.config.write().unwrap();
        config.routing.rules = new_rules.iter().map(|r| crate::config::ModelRuleConfig {
            pattern: r.pattern.clone(),
            target_provider: r.target_provider.clone(),
        }).collect();
    }

    // Store in state
    *state.model_rules.write().unwrap() = new_rules;

    json_response(&serde_json::json!({ "rules": rules_json }))
}

// ============================================================================
// Stats Handlers
// ============================================================================

async fn handle_get_stats(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let (total_requests, total_errors) = state.provider_manager.get_total_stats();
    let all_stats = state.provider_manager.get_all_stats();

    let active_providers = all_stats.iter().filter(|s| s.enabled).count();
    let healthy_providers = all_stats.iter()
        .filter(|s| s.state == "healthy" && s.enabled)
        .count();
    let total_providers = all_stats.len();

    let config = state.config.read().unwrap();

    json_response(&serde_json::json!({
        "total_requests": total_requests,
        "requests_per_minute": state.rpm_counter.get_rpm(),
        "active_providers": active_providers,
        "healthy_providers": healthy_providers,
        "total_providers": total_providers,
        "strategy": config.routing.strategy,
        "uptime_secs": state.start_time.elapsed().as_secs()
    }))
}

async fn handle_get_timeseries(
    _uri: &hyper::Uri,
    _state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    let params = parse_query_params(_uri);
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("1h");

    json_response(&serde_json::json!({
        "period": period,
        "interval_secs": 60,
        "data": serde_json::Value::Array(vec![])
    }))
}

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

// ============================================================================
// Config Handlers
// ============================================================================

async fn handle_get_config(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    // Read raw config file
    let content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return error_response(&format!("Failed to read config: {}", e), hyper::StatusCode::INTERNAL_SERVER_ERROR),
    };

    let metadata = match std::fs::metadata(&state.config_path) {
        Ok(m) => m,
        Err(_) => return error_response("Failed to read config metadata", hyper::StatusCode::INTERNAL_SERVER_ERROR),
    };

    let last_modified = metadata.modified()
        .ok()
        .and_then(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            Some(datetime.to_rfc3339())
        })
        .unwrap_or_default();

    json_response(&serde_json::json!({
        "content": content,
        "path": state.config_path,
        "last_modified": last_modified,
        "last_reloaded": last_modified  // Could track separately
    }))
}

async fn handle_update_config(
    req: hyper::Request<Incoming>,
    state: &crate::proxy::AppState,
) -> hyper::Response<ResponseBody> {
    // Collect body
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
    };

    #[derive(serde::Deserialize)]
    struct UpdateConfigRequest {
        content: String,
    }

    let update_req: UpdateConfigRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

    // Validate by parsing
    match crate::config::Config::load_from_str(&update_req.content) {
        Ok(_) => {},
        Err(e) => {
            return json_response(&serde_json::json!({
                "saved": false,
                "error": format!("TOML parse error: {}", e)
            }));
        }
    }

    // Write to disk
    if let Err(e) = std::fs::write(&state.config_path, &update_req.content) {
        return error_response(&format!("Failed to write config: {}", e), hyper::StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Reload
    if let Err(e) = state.reload_config() {
        return json_response(&serde_json::json!({
            "saved": true,
            "reloaded": false,
            "error": e
        }));
    }

    let now = chrono::Utc::now().to_rfc3339();

    json_response(&serde_json::json!({
        "saved": true,
        "reloaded": true,
        "last_modified": now,
        "last_reloaded": now
    }))
}

async fn handle_validate_config(req: hyper::Request<Incoming>) -> hyper::Response<ResponseBody> {
    // Collect body
    let body_bytes = match req.collect().await {
        Ok(bytes) => bytes.to_bytes(),
        Err(_) => return bad_request_response("Failed to read request body"),
    };

    #[derive(serde::Deserialize)]
    struct ValidateConfigRequest {
        content: String,
    }

    let validate_req: ValidateConfigRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => return bad_request_response(&format!("Invalid JSON: {}", e)),
    };

    match crate::config::Config::load_from_str(&validate_req.content) {
        Ok(_) => json_response(&serde_json::json!({ "valid": true })),
        Err(e) => json_response(&serde_json::json!({
            "valid": false,
            "error": format!("{}", e)
        })),
    }
}

// ============================================================================
// System Handlers
// ============================================================================

async fn handle_health(state: &crate::proxy::AppState) -> hyper::Response<ResponseBody> {
    let providers = state.provider_manager.get_all_states();

    json_response(&serde_json::json!({
        "status": "ok",
        "uptime_secs": state.start_time.elapsed().as_secs(),
        "providers": providers
    }))
}

async fn handle_version() -> hyper::Response<ResponseBody> {
    json_response(&serde_json::json!({
        "version": "0.1.0",
        "build_time": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "rust_version": env!("CARGO_PKG_RUST_VERSION")
    }))
}
