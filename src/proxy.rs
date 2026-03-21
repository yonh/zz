use std::sync::Arc;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use std::collections::HashSet;

pub async fn proxy_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<hyper::Response<Full<Bytes>>, crate::error::ProxyError> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Collect request headers upfront
    let headers = req.headers().clone();

    // Collect request body upfront (needed for retries)
    let body_bytes = req.collect().await
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?
        .to_bytes();

    let max_retries = state.config.routing.max_retries;
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
        .body(Full::from(error_msg))
        .unwrap())
}

async fn attempt_request(
    provider_name: &str,
    provider: &Arc<crate::provider::Provider>,
    path: &str,
    method: &hyper::Method,
    headers: &hyper::HeaderMap,
    body_bytes: &Bytes,
    state: &AppState,
) -> Result<hyper::Response<Full<Bytes>>, crate::error::ProxyError> {
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
        .body(Full::from(body_bytes.clone()))
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Create HTTP client with timeout
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_only()
        .enable_http1()
        .build();

    let timeout = std::time::Duration::from_secs(state.config.server.request_timeout_secs);

    let client: hyper_util::client::legacy::Client<_, Full<Bytes>> =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .build(https);

    // Send request to upstream with timeout
    tracing::debug!(url = %upstream_url, "Sending request to upstream");

    let response = tokio::time::timeout(
        timeout,
        client.request(upstream_req)
    ).await
        .map_err(|_| crate::error::ProxyError::RequestError("Request timeout".to_string()))?
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?;

    let status = response.status();
    let response_headers = response.headers().clone();

    // Collect response body
    let response_bytes = response.collect().await
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
        .to_bytes();

    // Check for quota errors (inspect body for 403)
    let is_quota = if status == hyper::StatusCode::TOO_MANY_REQUESTS {
        true
    } else if status == hyper::StatusCode::FORBIDDEN {
        // Check body for quota keywords
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

    // Success or non-retryable error - return response
    // Build downstream response
    let mut downstream_response = hyper::Response::builder().status(status);
    for (name, value) in response_headers.iter() {
        downstream_response = downstream_response.header(name, value);
    }

    let response = downstream_response
        .body(Full::from(response_bytes))
        .unwrap();

    // Mark provider as healthy on success
    if status.is_success() {
        state.provider_manager.reset(provider_name);
    }

    Ok(response)
}

#[derive(Clone)]
pub struct AppState {
    pub provider_manager: Arc<crate::provider::ProviderManager>,
    pub router: Arc<crate::router::Router>,
    pub config: Arc<crate::config::Config>,
}