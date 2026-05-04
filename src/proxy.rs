use dashmap::DashMap;
use std::sync::Arc;
use http_body_util::{BodyExt, Full, combinators::BoxBody, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use std::collections::HashSet;
use tracing::Instrument;
use futures_util::TryStreamExt;
use std::pin::Pin;
use std::task::{Context, Poll};

type ResponseBody = BoxBody<Bytes, hyper::Error>;

/// Wrapper body that fires an `on_drop` callback when the SSE stream completes.
///
/// When hyper finishes streaming the response body to the client, this wrapper
/// is dropped. The drop handler spawns a task to finalize the journal entry with
/// the actual stream duration and total bytes.
struct SseStreamGuard<B> {
    inner: B,
    on_drop: std::sync::Mutex<Option<Box<dyn FnOnce(u64, Vec<u8>) + Send>>>,
    total_bytes: Arc<std::sync::atomic::AtomicU64>,
    /// Buffer accumulated from SSE chunks, capped at SSE_BODY_CAPTURE_CAP.
    body_buffer: Arc<std::sync::Mutex<Vec<u8>>>,
}

/// Maximum bytes of SSE response body to capture for journal inspection.
const SSE_BODY_CAPTURE_CAP: usize = 4096;

impl<B> SseStreamGuard<B> {
    fn new(
        inner: B,
        total_bytes: Arc<std::sync::atomic::AtomicU64>,
        body_buffer: Arc<std::sync::Mutex<Vec<u8>>>,
        on_drop: Box<dyn FnOnce(u64, Vec<u8>) + Send>,
    ) -> Self {
        Self {
            inner,
            on_drop: std::sync::Mutex::new(Some(on_drop)),
            total_bytes,
            body_buffer,
        }
    }
}

impl<B> Drop for SseStreamGuard<B> {
    fn drop(&mut self) {
        let bytes = self.total_bytes.load(std::sync::atomic::Ordering::Relaxed);
        let body = std::mem::take(&mut *self.body_buffer.lock().unwrap());
        if let Some(callback) = self.on_drop.lock().unwrap().take() {
            callback(bytes, body);
        }
    }
}

impl<B> hyper::body::Body for SseStreamGuard<B>
where
    B: hyper::body::Body<Data = Bytes, Error = hyper::Error> + Unpin,
{
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // SAFETY: we never move `inner` out; we only re-pin the projection.
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_frame(cx)
    }
}

pub type HttpClient = hyper_util::client::legacy::Client<
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
    http_body_util::combinators::BoxBody<Bytes, hyper::Error>,
>;

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

    // Collect request headers upfront
    let headers = req.headers().clone();

    // Collect request body upfront (needed for retries AND SSE detection)
    let body_bytes = req.collect().await
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?
        .to_bytes();

    // Detect SSE request: check Accept header OR "stream":true in body.
    // Many LLM clients (Codex, Cursor, etc.) omit the Accept header and
    // instead signal streaming via the JSON body field.
    let is_sse = is_sse_from_headers(&headers) || crate::stream::is_streaming_body(&body_bytes);

    // Tracking variables for logging
    let request_id = generate_request_id();
    let proxy_start = std::time::Instant::now();
    let request_bytes = body_bytes.len() as u64;

    // Read config once for timing toggle, max_retries, and request_timeout
    let (timing_enabled, max_retries, request_timeout_secs) = {
        let config = state.config.read().unwrap();
        (
            config.observability.timing.enabled,
            config.routing.max_retries,
            config.server.request_timeout_secs,
        )
    };

    // Timing state
    let mut timing = if timing_enabled {
        Some(crate::request_journal::RequestTiming::default())
    } else {
        None
    };

    // Root tracing span — wraps the entire request body so that
    // #[instrument] child spans parent to it correctly.
    let root_span = tracing::info_span!(
        "proxy_handler",
        request_id = %request_id,
        method = %method,
        path = %path,
    );

    // Use .instrument() to enter the span for the async body.
    // This is the async-safe equivalent of root_span.enter().
    async move {
    // 1. Model parsing + pre-filter timing
    let t_parse = std::time::Instant::now();
    let model = extract_model(&body_bytes);
    let base_providers: Vec<_> = if model != "unknown" {
        let model_providers = state.provider_manager.get_available_for_model(&model);
        if model_providers.is_empty() {
            // Check if any provider even declares support for this model
            let has_support = state.provider_manager.get_all_stats().iter()
                .any(|s| state.provider_manager.get_by_name(&s.name)
                    .map(|p| p.supports_model(&model))
                    .unwrap_or(false));

            if !has_support {
                tracing::warn!(model = %model, "No provider configured to support this model");
                // Finalize timing for early exit
                if let Some(ref mut t) = timing {
                    t.parse_model_ms = t_parse.elapsed().as_millis() as u64;
                    t.available_providers = 0;
                    t.completed = false;
                }
                let error_body = serde_json::json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": format!("No provider is configured to support model: {}", model)
                    }
                });
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::BAD_REQUEST)
                    .header("content-type", "application/json")
                    .body(full(error_body.to_string()))
                    .unwrap());
            }
            tracing::warn!(
                model = %model,
                "All providers supporting this model are currently unavailable"
            );
        }
        model_providers
    } else {
        state.provider_manager.get_available()
    };

    if let Some(ref mut t) = timing {
        t.parse_model_ms = t_parse.elapsed().as_millis() as u64;
        t.available_providers = base_providers.len() as u16;
    }

    let mut failover_chain: Vec<String> = Vec::new();
    let mut final_provider = String::new();
    let mut final_upstream_url = String::new();
    let mut final_status: u16 = 503;
    let mut response_bytes: u64 = 0;
    let mut ttfb_ms: u64 = 0;

    let mut tried_providers: HashSet<String> = HashSet::new();
    let mut last_error: Option<crate::error::ProxyError> = None;

    // Cumulative timing across retries
    let mut total_select_ms: u64 = 0;
    let mut total_upstream_ms: u64 = 0;
    let mut retry_count: u8 = 0;

    for _ in 0..max_retries {
        // Get providers (excluding already tried ones)
        let providers: Vec<_> = base_providers.iter()
            .filter(|(name, _)| !tried_providers.contains(name))
            .cloned()
            .collect();

        if providers.is_empty() {
            break;
        }

        // 2. Provider selection timing
        let t_select = std::time::Instant::now();
        let sel = {
            // 1. Check model pinning (highest priority)
            if let Some(pinned_provider_name) = state.model_pins.get(&model) {
                let pinned_name = pinned_provider_name.value().clone();
                drop(pinned_provider_name);

                match state.provider_manager.get_by_name(&pinned_name) {
                    Some(p) if p.is_available() => {
                        tracing::info!(
                            model = %model,
                            pinned_provider = %pinned_name,
                            "Using pinned provider for model"
                        );
                        ProviderSelection {
                            provider_name: pinned_name.clone(),
                            provider: Arc::clone(&p),
                            is_pinned: true,
                            selection_reason: format!("pinned:{}", pinned_name),
                        }
                    }
                    Some(_) => {
                        // Pinned provider exists but is unavailable (disabled/unhealthy)
                        tracing::error!(
                            model = %model,
                            pinned_provider = %pinned_name,
                            "Pinned provider is unavailable, returning 503"
                        );
                        // Finalize timing for pinned-unavailable path
                        if let Some(ref mut t) = timing {
                            t.selection_reason = format!("pinned:{}", pinned_name);
                            t.completed = false;
                        }
                        write_request_journal(
                            &state,
                            JournalParams {
                                request_id: request_id.clone(),
                                headers: &headers,
                                body_bytes: &body_bytes,
                                method: &method,
                                path: &path,
                                provider: pinned_name.clone(),
                                upstream_url: String::new(),
                                model: model.clone(),
                                streaming: is_sse,
                                status: 503,
                                request_bytes,
                                response_bytes: 0,
                                failover_chain: None,
                                error: Some(format!("Pinned provider '{}' for model '{}' is unavailable", pinned_name, model)),
                                upstream_error_body: None,
                                upstream_response_body: None,
                                timing,
                            },
                        );
                        return Err(crate::error::ProxyError::ProviderError(format!(
                            "Pinned provider '{}' for model '{}' is unavailable",
                            pinned_name, model
                        )));
                    }
                    None => {
                        // Pinned provider was deleted, clean up pin and fall through
                        tracing::warn!(
                            model = %model,
                            pinned_provider = %pinned_name,
                            "Pinned provider deleted, removing pin"
                        );
                        state.model_pins.remove(&model);
                        match select_provider_normal(&model, &providers, &state) {
                            Some(s) => s,
                            None => break,
                        }
                    }
                }
            } else {
                // No pin, use normal routing
                match select_provider_normal(&model, &providers, &state) {
                    Some(s) => s,
                    None => break,
                }
            }
        };
        total_select_ms += t_select.elapsed().as_millis() as u64;

        // Record selection reason
        if let Some(ref mut t) = timing {
            t.selection_reason = sel.selection_reason.clone();
            if t.selection_reason.len() > 128 {
                t.selection_reason.truncate(128);
            }
        }

        let provider_name = sel.provider_name;
        let provider = sel.provider;
        let is_pinned = sel.is_pinned;

        tried_providers.insert(provider_name.clone());

        tracing::info!(
            provider = %provider_name,
            method = %method,
            path = %path,
            is_sse = is_sse,
            "Selected provider for request"
        );

        // Increment request counter
        state.provider_manager.increment_request(&provider_name);

        // 3. Upstream request timing
        let t_attempt = std::time::Instant::now();

        // Attempt request with this provider
        match attempt_request(
            &state.http_client,
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
            Ok((response, resp_bytes, resp_ttfb_ms, token_usage, upstream_url, sse_total_bytes, sse_body_buffer, upstream_error_body, upstream_response_body)) => {
                let attempt_ms = t_attempt.elapsed().as_millis() as u64;
                total_upstream_ms += attempt_ms;
                let status_code = response.status().as_u16();
                failover_chain.push(format!("{}:{}", provider_name, status_code));
                final_provider = provider_name.clone();
                final_upstream_url = upstream_url;
                final_status = status_code;
                response_bytes = resp_bytes;
                ttfb_ms = resp_ttfb_ms;

                let latency_ms = proxy_start.elapsed().as_millis() as u64;
                provider.record_latency(latency_ms);

                if let Some(ref usage) = token_usage {
                    state.provider_manager.record_tokens(
                        &provider_name,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                    );
                }

                // Finalize timing
                // For SSE: mark timing as incomplete — will be finalized
                // when the stream body is dropped.
                if let Some(ref mut t) = timing {
                    t.select_provider_ms = total_select_ms;
                    t.upstream_total_ms = total_upstream_ms;
                    t.upstream_ttfb_ms = resp_ttfb_ms;
                    t.retry_count = retry_count;
                    t.retry_providers.push(provider_name.clone());
                    t.retry_durations_ms.push(attempt_ms);
                    t.retry_errors.push("ok".to_string());
                    t.completed = !is_sse;
                }

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
                    token_usage,
                };
                state.log_buffer.push(log_entry.clone());
                state.rpm_counter.increment();
                state.ws_broadcaster.broadcast_log(log_entry);

                // Capture journal timestamp before writing entry
                let journal_timestamp = chrono::Utc::now().to_rfc3339();
                let stream_start = std::time::Instant::now();

                write_request_journal(
                    &state,
                    JournalParams {
                        request_id: request_id.clone(),
                        headers: &headers,
                        body_bytes: &body_bytes,
                        method: &method,
                        path: &path,
                        provider: final_provider.clone(),
                        upstream_url: final_upstream_url.clone(),
                        model: model.clone(),
                        streaming: is_sse,
                        status: final_status,
                        request_bytes,
                        response_bytes,
                        failover_chain: if failover_chain.len() > 1 { Some(failover_chain.clone()) } else { None },
                        error: None,
                        upstream_error_body,
                        upstream_response_body,
                        timing,
                    },
                );

                // For SSE responses, wrap the body with a drop guard that
                // finalizes timing when the stream completes.
                if is_sse {
                    let journal = state.request_journal.clone();
                    let entry_id = request_id.clone();
                    let total_bytes = sse_total_bytes.unwrap();
                    let body_buffer = sse_body_buffer.unwrap();
                    let (parts, body) = response.into_parts();
                    let guard = SseStreamGuard::new(
                        body,
                        total_bytes,
                        body_buffer,
                        Box::new(move |final_bytes: u64, captured_body: Vec<u8>| {
                            let stream_duration_ms = stream_start.elapsed().as_millis() as u64;
                            tracing::debug!(
                                id = %entry_id,
                                stream_duration_ms,
                                total_bytes = final_bytes,
                                captured_body_len = captured_body.len(),
                                "SSE stream completed, finalizing journal entry"
                            );
                            tokio::spawn(async move {
                                journal
                                    .finalize_sse_entry(
                                        &entry_id,
                                        &journal_timestamp,
                                        final_bytes,
                                        stream_duration_ms,
                                        captured_body,
                                    )
                                    .await;
                            });
                        }),
                    );
                    return Ok(hyper::Response::from_parts(parts, guard.boxed()));
                }

                return Ok(response);
            }
            Err(e) => {
                let attempt_ms = t_attempt.elapsed().as_millis() as u64;
                total_upstream_ms += attempt_ms;

                failover_chain.push(format!("{}:err", provider_name));
                final_provider = provider_name.clone();

                // Record failed attempt in timing
                if let Some(ref mut t) = timing {
                    t.retry_providers.push(provider_name.clone());
                    t.retry_durations_ms.push(attempt_ms);
                    t.retry_errors.push(classify_error(&e).to_string());
                }

                if is_pinned {
                    tracing::error!(
                        provider = %provider_name,
                        error = %e,
                        "Pinned provider failed, returning error without retry"
                    );

                    // Finalize timing for pinned failure
                    if let Some(ref mut t) = timing {
                        t.select_provider_ms = total_select_ms;
                        t.upstream_total_ms = total_upstream_ms;
                        t.retry_count = retry_count;
                        t.completed = false;
                    }

                    write_request_journal(
                        &state,
                        JournalParams {
                            request_id,
                            headers: &headers,
                            body_bytes: &body_bytes,
                            method: &method,
                            path: &path,
                            provider: final_provider.clone(),
                            upstream_url: String::new(),
                            model,
                            streaming: is_sse,
                            status: 503,
                            request_bytes,
                            response_bytes: 0,
                            failover_chain: if failover_chain.len() > 1 { Some(failover_chain) } else { None },
                            error: Some(e.to_string()),
                            upstream_error_body: None,
                            upstream_response_body: None,
                            timing,
                        },
                    );

                    return Err(e);
                }

                tracing::warn!(
                    provider = %provider_name,
                    error = %e,
                    "Request failed, will try next provider"
                );
                retry_count += 1;
                last_error = Some(e);
            }
        }
    }

    // All providers failed — finalize timing
    if let Some(ref mut t) = timing {
        t.select_provider_ms = total_select_ms;
        t.upstream_total_ms = total_upstream_ms;
        t.retry_count = retry_count;
        t.completed = false;
    }

    let error_msg = last_error
        .map(|e| format!("All providers failed. Last error: {}", e))
        .unwrap_or_else(|| "No available providers".to_string());

    // Log failed request
    let log_entry = crate::stats::LogEntry {
        id: request_id.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        method: method.to_string(),
        path: path.clone(),
        provider: final_provider.clone(),
        status: 503,
        duration_ms: proxy_start.elapsed().as_millis() as u64,
        ttfb_ms: 0,
        model: model.clone(),
        streaming: is_sse,
        request_bytes,
        response_bytes: 0,
        failover_chain: if failover_chain.len() > 1 { Some(failover_chain.clone()) } else { None },
        token_usage: None,
    };
    state.log_buffer.push(log_entry.clone());
    state.rpm_counter.increment();
    state.ws_broadcaster.broadcast_log(log_entry);

    write_request_journal(
        &state,
        JournalParams {
            request_id,
            headers: &headers,
            body_bytes: &body_bytes,
            method: &method,
            path: &path,
            provider: final_provider.clone(),
            upstream_url: final_upstream_url.clone(),
            model,
            streaming: is_sse,
            status: 503,
            request_bytes,
            response_bytes: 0,
            failover_chain: if failover_chain.len() > 1 { Some(failover_chain) } else { None },
            error: Some(error_msg.clone()),
            upstream_error_body: None,
            upstream_response_body: None,
            timing,
        },
    );

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SERVICE_UNAVAILABLE)
        .body(full(error_msg))
        .unwrap())
    }
    .instrument(root_span)
    .await
}

/// Check Accept header for SSE from an already-cloned HeaderMap.
fn is_sse_from_headers(headers: &hyper::HeaderMap) -> bool {
    if let Some(accept) = headers.get(hyper::header::ACCEPT) {
        if let Ok(accept_str) = accept.to_str() {
            if accept_str.contains("text/event-stream") {
                return true;
            }
        }
    }
    false
}

#[tracing::instrument(name = "attempt_request", skip(provider, headers, body_bytes, http_client, state), fields(provider = %provider_name))]
async fn attempt_request(
    http_client: &HttpClient,
    provider_name: &str,
    provider: &Arc<crate::provider::Provider>,
    path: &str,
    method: &hyper::Method,
    headers: &hyper::HeaderMap,
    body_bytes: &Bytes,
    state: &AppState,
    is_sse: bool,
    request_timeout_secs: u64,
) -> Result<(hyper::Response<ResponseBody>, u64, u64, Option<crate::stats::TokenUsage>, String, Option<Arc<std::sync::atomic::AtomicU64>>, Option<Arc<std::sync::Mutex<Vec<u8>>>>, Option<String>, Option<String>), crate::error::ProxyError> {
    // Returns: (response, response_bytes, ttfb_ms, token_usage, upstream_url, sse_total_bytes, sse_body_buffer, upstream_error_body, upstream_response_body)
    // sse_total_bytes is Some(arc) for SSE responses, None otherwise.
    // sse_body_buffer is Some(arc) for SSE responses, None otherwise.
    // upstream_error_body is Some(truncated body) for non-2xx responses, None otherwise.
    // upstream_response_body is Some(truncated body) for non-SSE 2xx responses, None otherwise.
    // Extract config values (needed to avoid holding RwLock across await)
    let (base_url, api_key, extra_headers) = {
        let config = provider.config.read().unwrap();
        (config.base_url.clone(), config.api_key.clone(), config.headers.clone())
    };

    // Rewrite URL
    let upstream_url = crate::rewriter::RequestRewriter::rewrite_url(
        &base_url,
        path,
    ).map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    // Rewrite headers
    let rewritten_headers = crate::rewriter::RequestRewriter::rewrite_headers(
        headers,
        &api_key,
        &base_url,
        &extra_headers,
    );

    // Build upstream request
    let mut upstream_req_builder = hyper::Request::builder()
        .method(method.clone())
        .uri(&upstream_url);

    for (name, value) in rewritten_headers.iter() {
        upstream_req_builder = upstream_req_builder.header(name, value);
    }

    let upstream_req = upstream_req_builder
        .body(Full::new(body_bytes.clone()).map_err(|never| match never {}).boxed())
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?;

    let timeout = std::time::Duration::from_secs(request_timeout_secs);

    // Send request to upstream with timeout using shared connection pool
    tracing::debug!(url = %upstream_url, is_sse = is_sse, "Sending request to upstream");

    let ttfb_start = std::time::Instant::now();
    let response = tokio::time::timeout(
        timeout,
        http_client.request(upstream_req)
    ).await
        .map_err(|_| crate::error::ProxyError::RequestError("Request timeout".to_string()))?
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?;

    let ttfb_ms = ttfb_start.elapsed().as_millis() as u64;
    let elapsed = ttfb_start.elapsed();

    let status = response.status();
    let response_headers = response.headers().clone();

    // For non-2xx responses, check for errors
    if !status.is_success() {
        // Collect response body for error checking
        let response_bytes = response.collect().await
            .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
            .to_bytes();

        // Truncate error body for diagnostic capture (max 4KB UTF-8)
        let error_body_snippet = |bytes: &Bytes| -> Option<String> {
            let end = bytes.len().min(4096);
            std::str::from_utf8(&bytes[..end]).ok().map(|s| s.to_string())
        };

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
            let body = error_body_snippet(&response_bytes)
                .unwrap_or_else(|| "<non-utf8 body>".to_string());
            return Err(crate::error::ProxyError::QuotaExhausted(
                format!("{} | {}", provider_name, body),
            ));
        }

        // Check for 5xx errors - should retry
        if status.as_u16() >= 500 && status.as_u16() < 600 {
            state.provider_manager.mark_failure(provider_name);
            let body = error_body_snippet(&response_bytes)
                .unwrap_or_else(|| "<non-utf8 body>".to_string());
            return Err(crate::error::ProxyError::HttpError(
                format!("Upstream error: {} | {}", status, body),
            ));
        }

        // Non-retryable error - return response to client
        let upstream_error_body = error_body_snippet(&response_bytes);
        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        let resp_bytes = response_bytes.len() as u64;
        let token_usage = extract_token_usage(&response_bytes);
        return Ok((downstream_response
            .body(full(response_bytes))
            .unwrap(), resp_bytes, ttfb_ms, token_usage, upstream_url, None, None, upstream_error_body, None));
    }

    // Success response
    state.provider_manager.reset(provider_name);

    tracing::info!(
        provider = %provider_name,
        status = %status.as_u16(),
        elapsed_ms = elapsed.as_millis(),
        is_sse = is_sse,
        "Request completed"
    );

    if is_sse {
        // Stream response for SSE
        tracing::info!("Streaming SSE response");

        // Track total SSE stream bytes via shared counter and accumulate body for journal.
        let total_bytes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0u64));
        let body_buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));

        let total_bytes_clone = total_bytes.clone();
        let body_buffer_clone = body_buffer.clone();
        let stream = response.into_data_stream()
            .inspect_ok(move |chunk: &Bytes| {
                total_bytes_clone.fetch_add(chunk.len() as u64, std::sync::atomic::Ordering::Relaxed);
                // Accumulate chunks into body buffer, capped at SSE_BODY_CAPTURE_CAP.
                {
                    let mut buf = body_buffer_clone.lock().unwrap();
                    if buf.len() < SSE_BODY_CAPTURE_CAP {
                        let remaining = SSE_BODY_CAPTURE_CAP - buf.len();
                        let end = chunk.len().min(remaining);
                        buf.extend_from_slice(&chunk[..end]);
                    }
                }
            })
            .map_ok(Frame::data);

        let body = StreamBody::new(stream).boxed();

        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        // For SSE: upstream_total_ms = TTFB (time to first chunk from upstream).
        // The actual stream duration will be measured via the on_drop callback
        // in proxy_handler. total_bytes and body_buffer are tracked via the inspector above.
        Ok((downstream_response
            .body(body)
            .unwrap(), 0, ttfb_ms, None, upstream_url, Some(total_bytes), Some(body_buffer), None, None))
    } else {
        // Buffer response for non-SSE
        let resp_bytes = response.collect().await
            .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
            .to_bytes();

        let response_bytes_val = resp_bytes.len() as u64;

        // Extract token usage from response
        let token_usage = extract_token_usage(&resp_bytes);

        let mut downstream_response = hyper::Response::builder().status(status);
        for (name, value) in response_headers.iter() {
            downstream_response = downstream_response.header(name, value);
        }

        // Capture response body for journal (truncate to max bytes, UTF-8 only)
        let upstream_response_body = {
            let cfg = state.config.read().unwrap();
            let rj = &cfg.observability.request_journal;
            if rj.capture_response_body {
                let max_bytes = rj.max_response_body_bytes as usize;
                let end = resp_bytes.len().min(max_bytes);
                std::str::from_utf8(&resp_bytes[..end]).ok().map(|s| s.to_string())
            } else {
                None
            }
        };

        Ok((downstream_response
            .body(full(resp_bytes))
            .unwrap(), response_bytes_val, ttfb_ms, token_usage, upstream_url, None, None, None, upstream_response_body))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub provider_manager: Arc<crate::provider::ProviderManager>,
    pub router: Arc<crate::router::Router>,
    pub config: Arc<std::sync::RwLock<crate::config::Config>>,
    pub config_path: String,
    pub start_time: std::time::Instant,
    pub log_buffer: Arc<crate::stats::RequestLogBuffer>,
    pub ws_broadcaster: Arc<crate::ws::WsBroadcaster>,
    pub model_rules: Arc<std::sync::RwLock<Vec<crate::router::ModelRule>>>,
    pub model_pins: Arc<DashMap<String, String>>,
    pub rpm_counter: Arc<crate::stats::RpmCounter>,
    pub request_journal: Arc<crate::request_journal::RequestJournalWriter>,
    pub last_reloaded: Arc<std::sync::Mutex<Option<String>>>,
    pub trace_sampler: Option<Arc<crate::trace_layer::TraceSampler>>,
    pub http_client: HttpClient,
}

impl AppState {
    pub fn reload_config(&self) -> Result<(), String> {
        let new_config = crate::config::Config::load(&self.config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?;

        self.provider_manager.reload(&new_config);

        let new_rules: Vec<crate::router::ModelRule> = new_config.routing.rules
            .iter()
            .enumerate()
            .map(|(i, r)| crate::router::ModelRule {
                id: format!("rule_{}", i + 1),
                pattern: r.pattern.clone(),
                target_provider: r.target_provider.clone(),
            })
            .collect();
        *self.model_rules.write().unwrap() = new_rules;

        let journal_config = new_config.observability.request_journal.clone();
        let new_tracing_config = new_config.observability.tracing.clone();
        *self.config.write().unwrap() = new_config;

        self.request_journal.update_config(journal_config);

        if let Some(sampler) = &self.trace_sampler {
            sampler.update_config(new_tracing_config);
        }
        
        let now = chrono::Utc::now().to_rfc3339();
        *self.last_reloaded.lock().unwrap() = Some(now);
        tracing::info!("Config reloaded successfully");
        Ok(())
    }
}

/// Result of provider selection with model rules.
enum SelectResult {
    /// A matching provider was found, along with the rule pattern that matched.
    Provider((String, Arc<crate::provider::Provider>), String),
    /// A rule matched, but the designated provider is not currently available.
    RuleMatchedButUnavailable(String, String),
    /// No rule matched — caller should fall back to default strategy.
    NoRule,
}

/// Provider selection result including the routing decision reason.
struct ProviderSelection {
    provider_name: String,
    provider: Arc<crate::provider::Provider>,
    is_pinned: bool,
    /// Routing decision reason — prefixed: pinned:|rule:|strategy:
    selection_reason: String,
}

/// Normal provider selection (rules + strategy) without pinning.
/// Returns None if rule matched but target is unavailable (should stop retry).
#[tracing::instrument(name = "select_provider_normal", skip(providers, state), fields(model = %model))]
fn select_provider_normal(
    model: &str,
    providers: &[(String, Arc<crate::provider::Provider>)],
    state: &AppState,
) -> Option<ProviderSelection> {
    let rules = state.model_rules.read().unwrap();
    tracing::debug!(
        model = %model,
        rules_count = rules.len(),
        "Selecting provider for model"
    );
    match select_with_rules(model, providers, &rules) {
        SelectResult::Provider((name, p), pattern) => {
            let reason = format!("rule:{}→{}", pattern, name);
            Some(ProviderSelection {
                provider_name: name,
                provider: p,
                is_pinned: false,
                selection_reason: reason,
            })
        }
        SelectResult::RuleMatchedButUnavailable(target, pattern) => {
            tracing::warn!(
                model = %model,
                target_provider = %target,
                pattern = %pattern,
                "Model rule matched but target provider is unavailable, stopping retry"
            );
            None
        }
        SelectResult::NoRule => {
            let strategy = state.config.read().unwrap().routing.strategy.clone();
            state.router.select_provider(providers).map(|(name, p)| {
                ProviderSelection {
                    provider_name: name,
                    provider: p,
                    is_pinned: false,
                    selection_reason: format!("strategy:{}", strategy),
                }
            })
        }
    }
}

fn select_with_rules(
    model: &str,
    providers: &[(String, Arc<crate::provider::Provider>)],
    rules: &[crate::router::ModelRule],
) -> SelectResult {
    for rule in rules {
        if crate::router::glob_match_pub(&rule.pattern, model) {
            tracing::info!(
                model = %model,
                pattern = %rule.pattern,
                target = %rule.target_provider,
                "Model rule matched"
            );
            return match providers.iter().find(|(name, _)| name == &rule.target_provider) {
                Some((name, p)) => SelectResult::Provider((name.clone(), Arc::clone(p)), rule.pattern.clone()),
                None => SelectResult::RuleMatchedButUnavailable(rule.target_provider.clone(), rule.pattern.clone()),
            };
        }
    }
    SelectResult::NoRule
}


#[tracing::instrument(name = "extract_model", skip(body))]
fn extract_model(body: &[u8]) -> String {
    // Try to parse as UTF-8 first
    let Ok(s) = std::str::from_utf8(body) else {
        tracing::debug!("Request body is not valid UTF-8");
        return "unknown".to_string();
    };

    // Try to parse as JSON (works for small/medium bodies)
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
            return m.to_string();
        }
        tracing::debug!("No 'model' field in request JSON");
        return "unknown".to_string();
    }

    // For large bodies or partial JSON, search for "model" field directly
    // This handles cases where full JSON parsing fails or is too expensive
    tracing::debug!("Full JSON parse failed, searching for model field directly");

    // Look for "model": "value" pattern
    if let Some(model) = extract_model_from_partial_json(s) {
        return model;
    }

    "unknown".to_string()
}

/// Extract model value by searching for "model": "value" pattern in JSON string.
/// This is a fallback for when full JSON parsing fails.
fn extract_model_from_partial_json(s: &str) -> Option<String> {
    // Find "model" key with various whitespace patterns
    let patterns = ["\"model\"", "'model'"];

    for pattern in patterns {
        let mut pos = 0;
        while let Some(idx) = s[pos..].find(pattern) {
            let key_end = pos + idx + pattern.len();

            // Skip whitespace and find colon
            let after_key = &s[key_end..];
            let colon_pos = after_key.find(':')?;

            // Skip whitespace after colon
            let after_colon = &after_key[colon_pos + 1..];
            let trimmed = after_colon.trim_start();

            // Extract quoted string value
            if trimmed.starts_with('"') {
                if let Some(end) = trimmed[1..].find('"') {
                    let value = &trimmed[1..end + 1];
                    return Some(value.to_string());
                }
            } else if trimmed.starts_with('\'') {
                if let Some(end) = trimmed[1..].find('\'') {
                    let value = &trimmed[1..end + 1];
                    return Some(value.to_string());
                }
            }

            pos = key_end;
        }
    }

    None
}

/// Generate a short random request ID.
fn generate_request_id() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let id: String = (0..12)
        .map(|_| {
            let idx = rng.random_range(0..36u8);
            if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
        })
        .collect();
    format!("req_{}", id)
}

/// Classify a ProxyError into a short label for retry tracking.
fn classify_error(e: &crate::error::ProxyError) -> &'static str {
    match e {
        crate::error::ProxyError::RequestError(msg) => {
            let lower = msg.to_lowercase();
            if lower.contains("timeout") {
                "timeout"
            } else if lower.contains("connection") || lower.contains("connect") || lower.contains("refused") {
                "connection"
            } else {
                "error"
            }
        }
        crate::error::ProxyError::QuotaExhausted(_) => "quota",
        crate::error::ProxyError::HttpError(msg) => {
            let lower = msg.to_lowercase();
            if lower.contains("429") {
                "429"
            } else if lower.contains("upstream error") {
                // 5xx from upstream: "Upstream error: 502 Bad Gateway"
                "5xx"
            } else {
                "http_4xx"
            }
        }
        crate::error::ProxyError::ProviderError(_) => "provider",
        crate::error::ProxyError::ConfigError(_) => "config",
        crate::error::ProxyError::AllProvidersFailed(_) => "all_failed",
    }
}

/// Extract token usage from API response body.
/// Supports OpenAI-style and Anthropic-style usage objects.
fn extract_token_usage(body: &[u8]) -> Option<crate::stats::TokenUsage> {
    // Try to parse as UTF-8
    let Ok(s) = std::str::from_utf8(body) else {
        return None;
    };

    // Try to parse as JSON
    let Ok(v) = serde_json::from_str::<serde_json::Value>(s) else {
        return None;
    };

    // Try OpenAI-style usage object: { "usage": { "prompt_tokens": N, "completion_tokens": M, "total_tokens": T } }
    if let Some(usage) = v.get("usage") {
        return extract_usage_from_object(usage);
    }

    // Try Anthropic-style: usage at top level with different field names
    // Anthropic: { "usage": { "input_tokens": N, "output_tokens": M } }
    if let Some(usage) = v.get("usage") {
        let prompt = usage.get("input_tokens").and_then(|v| v.as_u64())
            .or_else(|| usage.get("prompt_tokens").and_then(|v| v.as_u64()));
        let completion = usage.get("output_tokens").and_then(|v| v.as_u64())
            .or_else(|| usage.get("completion_tokens").and_then(|v| v.as_u64()));

        if let (Some(p), Some(c)) = (prompt, completion) {
            let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(p + c);

            // Extract additional details if present
            let details = extract_usage_details(usage);

            return Some(crate::stats::TokenUsage {
                prompt_tokens: p,
                completion_tokens: c,
                total_tokens: total,
                details,
            });
        }
    }

    // Some APIs return usage directly in response (not nested)
    if v.get("prompt_tokens").is_some() || v.get("input_tokens").is_some() {
        return extract_usage_from_object(&v);
    }

    None
}

/// Extract usage from a usage object (OpenAI or Anthropic style)
fn extract_usage_from_object(usage: &serde_json::Value) -> Option<crate::stats::TokenUsage> {
    let prompt = usage.get("prompt_tokens").and_then(|v| v.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()));
    let completion = usage.get("completion_tokens").and_then(|v| v.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()));

    match (prompt, completion) {
        (Some(p), Some(c)) => {
            let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(p + c);
            let details = extract_usage_details(usage);
            Some(crate::stats::TokenUsage {
                prompt_tokens: p,
                completion_tokens: c,
                total_tokens: total,
                details,
            })
        }
        (Some(p), None) => {
            // Only prompt tokens available (might be embedding request)
            let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(p);
            Some(crate::stats::TokenUsage {
                prompt_tokens: p,
                completion_tokens: 0,
                total_tokens: total,
                details: None,
            })
        }
        _ => None,
    }
}

/// Extract additional usage details (cached tokens, reasoning tokens, etc.)
fn extract_usage_details(usage: &serde_json::Value) -> Option<serde_json::Value> {
    let mut details = serde_json::Map::new();

    // OpenAI cached tokens
    if let Some(cached) = usage.get("prompt_tokens_details").and_then(|v| v.get("cached_tokens")).and_then(|v| v.as_u64()) {
        details.insert("cached_tokens".to_string(), serde_json::Value::Number(cached.into()));
    }

    // OpenAI reasoning tokens
    if let Some(reasoning) = usage.get("completion_tokens_details").and_then(|v| v.get("reasoning_tokens")).and_then(|v| v.as_u64()) {
        details.insert("reasoning_tokens".to_string(), serde_json::Value::Number(reasoning.into()));
    }

    // Anthropic cache read/write
    if let Some(cache_read) = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()) {
        details.insert("cache_read_tokens".to_string(), serde_json::Value::Number(cache_read.into()));
    }
    if let Some(cache_write) = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()) {
        details.insert("cache_write_tokens".to_string(), serde_json::Value::Number(cache_write.into()));
    }

    if details.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(details))
    }
}

/// Parameters for writing a request journal entry, grouping the many fields
/// into a single struct to keep the function signature manageable.
struct JournalParams<'a> {
    request_id: String,
    headers: &'a hyper::HeaderMap,
    body_bytes: &'a Bytes,
    method: &'a hyper::Method,
    path: &'a str,
    provider: String,
    upstream_url: String,
    model: String,
    streaming: bool,
    status: u16,
    request_bytes: u64,
    response_bytes: u64,
    failover_chain: Option<Vec<String>>,
    error: Option<String>,
    upstream_error_body: Option<String>,
    upstream_response_body: Option<String>,
    timing: Option<crate::request_journal::RequestTiming>,
}

fn write_request_journal(
    state: &AppState,
    params: JournalParams<'_>,
) {
    if !state.request_journal.is_enabled() {
        return;
    }

    let user_agent = params.headers
        .get(hyper::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let client_name = crate::request_journal::infer_client_name(user_agent);
    let request_headers = state.request_journal.redact_headers(params.headers);

    let content_type = params.headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (request_body_text, request_body_base64) = if let Ok(text) = std::str::from_utf8(params.body_bytes) {
        (Some(text.to_string()), None)
    } else {
        (None, Some(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, params.body_bytes)))
    };

    let entry = crate::request_journal::RequestJournalEntry {
        id: params.request_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        client_name,
        user_agent: user_agent.to_string(),
        method: params.method.to_string(),
        path: params.path.to_string(),
        provider: params.provider,
        upstream_url: params.upstream_url,
        model: params.model,
        streaming: params.streaming,
        status: params.status,
        request_headers,
        request_content_type: content_type.to_string(),
        request_body_text,
        request_body_base64,
        request_bytes: params.request_bytes,
        response_bytes: params.response_bytes,
        failover_chain: params.failover_chain,
        error: params.error,
        upstream_error_body: params.upstream_error_body,
        response_body_text: params.upstream_response_body
            .as_deref()
            .and_then(crate::request_journal::extract_response_content)
            .or(params.upstream_response_body.clone()),
        response_body_base64: None,
        upstream_response_body: params.upstream_response_body,
        sse_raw_body: None,
        timing: params.timing,
    };

    let journal = state.request_journal.clone();
    tokio::spawn(async move {
        journal.write_entry(entry).await;
    });
}