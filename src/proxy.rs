use std::sync::Arc;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;

pub async fn proxy_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<hyper::Response<Full<Bytes>>, crate::error::ProxyError> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Get available providers
    let providers = state.provider_manager.get_available();
    if providers.is_empty() {
        return Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SERVICE_UNAVAILABLE)
            .body(Full::from("No available providers"))
            .unwrap());
    }

    // Select provider based on routing strategy
    let (provider_name, provider) = match state.router.select_provider(&providers) {
        Some(p) => p,
        None => {
            return Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SERVICE_UNAVAILABLE)
                .body(Full::from("No provider selected"))
                .unwrap());
        }
    };

    tracing::info!(
        provider = %provider_name,
        method = %method,
        path = %path,
        "Selected provider for request"
    );

    // Rewrite URL
    let upstream_url = crate::rewriter::RequestRewriter::rewrite_url(
        &provider.config.base_url,
        path,
    ).map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Collect request headers
    let headers = req.headers();
    let rewritten_headers = crate::rewriter::RequestRewriter::rewrite_headers(
        headers,
        &provider.config.api_key,
        &provider.config.base_url,
        &provider.config.headers,
    );

    // Collect request body
    let body_bytes = req.collect().await
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?
        .to_bytes();

    // Build upstream request
    let upstream_req = hyper::Request::builder()
        .method(method.clone())
        .uri(&upstream_url);

    let mut upstream_req_builder = upstream_req;
    for (name, value) in rewritten_headers.iter() {
        upstream_req_builder = upstream_req_builder.header(name, value);
    }

    let upstream_req = upstream_req_builder
        .body(Full::from(body_bytes))
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Create HTTP client
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_only()
        .enable_http1()
        .build();

    let client: hyper_util::client::legacy::Client<_, Full<Bytes>> =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build(https);

    // Send request to upstream
    tracing::debug!(url = %upstream_url, "Sending request to upstream");

    let response = client.request(upstream_req).await
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?;

    let status = response.status();
    let response_headers = response.headers().clone();

    // Check for quota errors
    if crate::error::is_quota_error(status, &[]) {
        tracing::warn!(provider = %provider_name, "Quota exhausted, marking provider as cooldown");
        state.provider_manager.mark_quota_exhausted(&provider_name);
    }

    // Collect response body
    let response_bytes = response.collect().await
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
        .to_bytes();

    // Build downstream response
    let mut downstream_response = hyper::Response::builder().status(status);

    for (name, value) in response_headers.iter() {
        downstream_response = downstream_response.header(name, value);
    }

    let response = downstream_response
        .body(Full::from(response_bytes))
        .unwrap();

    Ok(response)
}

#[derive(Clone)]
pub struct AppState {
    pub provider_manager: Arc<crate::provider::ProviderManager>,
    pub router: Arc<crate::router::Router>,
    pub config: Arc<crate::config::Config>,
}