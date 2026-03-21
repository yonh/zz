use std::sync::Arc;
use http_body_util::{BodyExt, Full, combinators::BoxBody, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use std::collections::HashSet;
use futures_util::TryStreamExt;

type ResponseBody = BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> ResponseBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

pub async fn proxy_handler(
    req: hyper::Request<Incoming>,
    state: AppState,
) -> Result<hyper::Response<ResponseBody>, crate::error::ProxyError> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Detect SSE request
    let is_sse = is_sse_request(&req);

    // Collect request headers upfront
    let headers = req.headers().clone();

    // Collect request body upfront (needed for retries)
    let body_bytes = req.collect().await
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?
        .to_bytes();

    let max_retries = {
        let config = state.config.read().unwrap();
        config.routing.max_retries
    };
    let request_timeout_secs = {
        let config = state.config.read().unwrap();
        config.server.request_timeout_secs
    };
    let mut tried_providers: HashSet<String> = HashSet::new();
    let mut last_error: Option<crate::error::ProxyError> = None;

    for _ in 0..max_retries {
        // Get available providers (excluding already tried ones)
        let providers: Vec<_> = state.provider_manager.get_available()
            .into_iter()
            .filter(|(name, _)| !tried_providers.contains(name))
            .collect();

        if providers.is_empty() {
            break;
        }

        // Select provider based on routing strategy
        let (provider_name, provider) = match state.router.select_provider(&providers) {
            Some(p) => p,
            None => break,
        };

        tried_providers.insert(provider_name.clone());

        tracing::info!(
            provider = %provider_name,
            method = %method,
            path = %path,
            is_sse = is_sse,
            "Selected provider for request"
        );

        // Attempt request with this provider
        match attempt_request(
            &provider_name,
            &provider,
            &path,
            &method,
            &headers,
            &body_bytes,
            &state,
            is_sse,
            request_timeout_secs,
        ).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!(
                    provider = %provider_name,
                    error = %e,
                    "Request failed, will try next provider"
                );
                last_error = Some(e);
            }
        }
    }

    // All providers failed
    let error_msg = last_error
        .map(|e| format!("All providers failed. Last error: {}", e))
        .unwrap_or_else(|| "No available providers".to_string());

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SERVICE_UNAVAILABLE)
        .body(full(error_msg))
        .unwrap())
}

fn is_sse_request(req: &hyper::Request<Incoming>) -> bool {
    // Check Accept header
    if let Some(accept) = req.headers().get(hyper::header::ACCEPT) {
        if let Ok(accept_str) = accept.to_str() {
            if accept_str.contains("text/event-stream") {
                return true;
            }
        }
    }
    false
}

async fn attempt_request(
    provider_name: &str,
    provider: &Arc<crate::provider::Provider>,
    path: &str,
    method: &hyper::Method,
    headers: &hyper::HeaderMap,
    body_bytes: &Bytes,
    state: &AppState,
    is_sse: bool,
    request_timeout_secs: u64,
) -> Result<hyper::Response<ResponseBody>, crate::error::ProxyError> {
    // Rewrite URL
    let upstream_url = crate::rewriter::RequestRewriter::rewrite_url(
        &provider.config.base_url,
        path,
    ).map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Rewrite headers
    let rewritten_headers = crate::rewriter::RequestRewriter::rewrite_headers(
        headers,
        &provider.config.api_key,
        &provider.config.base_url,
        &provider.config.headers,
    );

    // Build upstream request
    let mut upstream_req_builder = hyper::Request::builder()
        .method(method.clone())
        .uri(&upstream_url);

    for (name, value) in rewritten_headers.iter() {
        upstream_req_builder = upstream_req_builder.header(name, value);
    }

    let upstream_req = upstream_req_builder
        .body(Full::new(body_bytes.clone()))
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Create HTTP client with timeout
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_only()
        .enable_http1()
        .build();

    let timeout = std::time::Duration::from_secs(request_timeout_secs);

    let client: hyper_util::client::legacy::Client<_, Full<Bytes>> =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .build(https);

    // Send request to upstream with timeout
    tracing::debug!(url = %upstream_url, is_sse = is_sse, "Sending request to upstream");

    let response = tokio::time::timeout(
        timeout,
        client.request(upstream_req)
    ).await
        .map_err(|_| crate::error::ProxyError::RequestError("Request timeout".to_string()))?
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?;

    let status = response.status();
    let response_headers = response.headers().clone();

    // For non-2xx responses, check for errors
    if !status.is_success() {
        // Collect response body for error checking
        let response_bytes = response.collect().await
            .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
            .to_bytes();

        // Check for quota errors (inspect body for 403)
        let is_quota = if status == hyper::StatusCode::TOO_MANY_REQUESTS {
            true
        } else if status == hyper::StatusCode::FORBIDDEN {
            crate::error::is_quota_error(status, &response_bytes)
        } else {
            false
        };

        if is_quota {
            tracing::warn!(provider = %provider_name, "Quota exhausted, marking provider as cooldown");
            state.provider_manager.mark_quota_exhausted(provider_name);
            return Err(crate::error::ProxyError::QuotaExhausted(provider_name.to_string()));
        }

        // Check for 5xx errors - should retry
        if status.as_u16() >= 500 && status.as_u16() < 600 {
            state.provider_manager.mark_failure(provider_name);
            return Err(crate::error::ProxyError::HttpError(format!("Upstream error: {}", status)));
        }

        // Non-retryable error - return response
        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        return Ok(downstream_response
            .body(full(response_bytes))
            .unwrap());
    }

    // Success response
    state.provider_manager.reset(provider_name);

    if is_sse {
        // Stream response for SSE
        tracing::info!("Streaming SSE response");

        let stream = response.into_data_stream()
            .map_ok(Frame::data);

        let body = StreamBody::new(stream).boxed();

        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        Ok(downstream_response
            .body(body)
            .unwrap())
    } else {
        // Buffer response for non-SSE
        let response_bytes = response.collect().await
            .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
            .to_bytes();

        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        Ok(downstream_response
            .body(full(response_bytes))
            .unwrap())
    }
}

#[derive(Clone)]
pub struct AppState {
    pub provider_manager: Arc<crate::provider::ProviderManager>,
    pub router: Arc<crate::router::Router>,
    pub config: Arc<std::sync::RwLock<crate::config::Config>>,
    pub config_path: String,
}

impl AppState {
    pub fn reload_config(&self) -> Result<(), String> {
        let new_config = crate::config::Config::load(&self.config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?;

        // Update provider manager
        self.provider_manager.reload(&new_config);

        // Update config
        {
            let mut config = self.config.write().unwrap();
            *config = new_config;
        }

        tracing::info!("Config reloaded successfully");
        Ok(())
    }
}