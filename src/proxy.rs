use dashmap::DashMap;
use std::sync::Arc;
use http_body_util::{BodyExt, Full, combinators::BoxBody, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use std::collections::HashSet;
use tracing::Instrument;
use futures_util::{StreamExt, TryStreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::converter::ApiConverter;
use crate::converter::telemetry::TelemetryContext;
use tokio::sync::mpsc;

type ResponseBody = BoxBody<Bytes, hyper::Error>;

/// Streaming body that converts SSE chunks on-the-fly
///
/// This body wraps an upstream SSE stream and converts each chunk through
/// a StreamConverter before sending it to the client. This enables true
/// streaming with low TTFB instead of collecting the entire response first.
struct ConversionStreamBody {
    /// Receiver for converted chunks
    rx: mpsc::Receiver<Result<Bytes, hyper::Error>>,
}

impl ConversionStreamBody {
    /// Create a new ConversionStreamBody that spawns a background task
    /// to read from the upstream stream, convert chunks, and send them
    /// through the channel.
    fn new<B>(
        mut upstream_body: B,
        converter: Arc<std::sync::Mutex<crate::converter::stream::StreamConverter>>,
    ) -> Self
    where
        B: hyper::body::Body<Data = Bytes, Error = hyper::Error> + Send + Unpin + 'static,
    {
        let (tx, rx) = mpsc::channel(32); // Buffer up to 32 converted chunks

        // Spawn background task to handle streaming conversion
        tokio::spawn(async move {
            use http_body_util::BodyExt;
            
            loop {
                // Read frame from upstream (no lock held)
                let frame_result = upstream_body.frame().await;
                
                match frame_result {
                    Some(Ok(frame)) => {
                        let chunk_to_convert = frame.data_ref().map(|c| c.clone());
                        let is_end = frame.is_trailers() || (frame.is_data() && frame.data_ref().map_or(false, |d| d.is_empty()));
                        
                        // Convert the chunk (lock held only during conversion)
                        let converted_chunks = if let Some(chunk) = chunk_to_convert {
                            let mut converter = converter.lock().unwrap();
                            match converter.push(&chunk) {
                                Ok(chunks) => Some(chunks),
                                Err(e) => {
                                    tracing::error!(error = ?e, "Stream conversion failed");
                                    return;
                                }
                            }
                        } else {
                            None
                        };
                        
                        // Send converted chunks (no lock held)
                        if let Some(chunks) = converted_chunks {
                            for converted_bytes in chunks {
                                if tx.send(Ok(converted_bytes)).await.is_err() {
                                    // Client disconnected, stop processing
                                    return;
                                }
                            }
                        }
                        
                        // Check if this is the last frame
                        if is_end {
                            // Finalize conversion (lock held only during finalization)
                            let final_chunks = {
                                let mut converter = converter.lock().unwrap();
                                match converter.finalize() {
                                    Ok(chunks) => chunks,
                                    Err(e) => {
                                        tracing::error!(error = ?e, "Stream finalization failed");
                                        return;
                                    }
                                }
                            };
                            
                            // Send final chunks (no lock held)
                            for final_bytes in final_chunks {
                                if tx.send(Ok(final_bytes)).await.is_err() {
                                    return;
                                }
                            }
                            return;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "Upstream stream error");
                        let _ = tx.send(Err(e));
                        return;
                    }
                    None => {
                        // Stream ended, finalize conversion
                        let final_chunks = {
                            let mut converter = converter.lock().unwrap();
                            match converter.finalize() {
                                Ok(chunks) => chunks,
                                Err(e) => {
                                    tracing::error!(error = ?e, "Stream finalization failed");
                                    return;
                                }
                            }
                        };
                        
                        for final_bytes in final_chunks {
                            if tx.send(Ok(final_bytes)).await.is_err() {
                                return;
                            }
                        }
                        return;
                    }
                }
            }
        });

        Self { rx }
    }
}

impl hyper::body::Body for ConversionStreamBody {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                // Stream ended
                Poll::Ready(None)
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }
}

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

/// Conversion proxy handler for API format translation
///
/// Handles requests to /a2o/* and /o2a/* prefixes, converting between API formats.
/// Follows the flow specified in phase-P4 §2.
pub async fn conversion_proxy_handler(
    req: hyper::Request<Incoming>,
    state: AppState,
    source: crate::converter::ApiType,
    target: crate::converter::ApiType,
) -> Result<hyper::Response<ResponseBody>, crate::error::ProxyError> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Generate request ID for telemetry
    let request_id = generate_request_id();

    // Collect request headers upfront
    let headers = req.headers().clone();

    // Collect request body (full buffer for conversion)
    let body_bytes = req.collect().await
        .map_err(|e| crate::error::ProxyError::RequestError(e.to_string()))?
        .to_bytes();

    // Construct telemetry context
    let telemetry_ctx = if state.telemetry.is_enabled() {
        Some(std::sync::Arc::new(crate::converter::telemetry::RealTelemetry::new(
            state.telemetry.clone(),
            request_id.clone(),
            path.clone(),
            source,
            target,
        )))
    } else {
        None
    };

    // Step 1: Select request converter based on source API type and convert with telemetry
    let converted_body = match source {
        crate::converter::ApiType::Anthropic => {
            let converter = &crate::converter::AnthropicToOpenAIConverter;
            if let Some(ref ctx) = telemetry_ctx {
                converter.convert_request_with_ctx(&body_bytes, crate::converter::TargetQuirks::default(), ctx.as_ref())
            } else {
                converter.convert_request(&body_bytes, target)
            }
        }
        crate::converter::ApiType::OpenAIChat => {
            let converter = &crate::converter::OpenAIChatToAnthropicConverter;
            if let Some(ref ctx) = telemetry_ctx {
                converter.convert_request_with_ctx(&body_bytes, ctx.as_ref())
            } else {
                converter.convert_request(&body_bytes, target)
            }
        }
        crate::converter::ApiType::OpenAIResponses => {
            let converter = &crate::converter::OpenAIResponsesToChatConverter;
            converter.convert_request(&body_bytes, target)
        }
        _ => {
            return Ok(hyper::Response::builder()
                .status(hyper::StatusCode::BAD_REQUEST)
                .body(full("Unsupported source API type"))
                .unwrap());
        }
    };

    let converted_body = match converted_body {
        Ok(body) => {
            tracing::debug!(
                request_id = %request_id,
                converted_body = %String::from_utf8_lossy(&body),
                "Converted request body"
            );
            body
        }
        Err(conv_err) => {
            tracing::error!(
                error = ?conv_err,
                code = %conv_err.code,
                "Request conversion failed"
            );

            // Report error to telemetry if available
            if let Some(ref ctx) = telemetry_ctx {
                ctx.as_ref().report_error(&conv_err);
            }

            // Return error with conversion status headers
            let error_body = match source {
                crate::converter::ApiType::Anthropic => {
                    // Anthropic-style error
                    serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "invalid_request_error",
                            "message": format!("Request conversion failed: {}", conv_err.message)
                        }
                    })
                }
                _ => {
                    // OpenAI-style error
                    serde_json::json!({
                        "error": {
                            "message": format!("Request conversion failed: {}", conv_err.message),
                            "type": "invalid_request_error",
                            "code": conv_err.code
                        }
                    })
                }
            };

            return Ok(hyper::Response::builder()
                .status(hyper::StatusCode::BAD_GATEWAY)
                .header("content-type", "application/json")
                .header("X-Conversion-Status", "failed")
                .header("X-Conversion-Phase", "request")
                .header("X-Conversion-Error", conv_err.code)
                .body(full(error_body.to_string()))
                .unwrap());
        }
    };

    // Step 2: Strip Codex-specific headers for /r2c/ requests
    let mut headers = headers;
    if source == crate::converter::ApiType::OpenAIResponses {
        headers.remove("openai-beta");
        headers.remove("x-openai-subagent");
        headers.remove("x-openai-memgen-request");
    }

    // Remove content-length since conversion may change body size.
    // hyper's Full body will set it correctly.
    headers.remove(hyper::header::CONTENT_LENGTH);

    // Step 3: Convert path using target_path
    let target_path = match crate::converter::target_path(source, target, &path) {
        Ok(p) => p,
        Err(conv_err) => {
            tracing::error!(
                error = ?conv_err,
                code = %conv_err.code,
                "Path conversion failed"
            );

            let error_body = serde_json::json!({
                "error": {
                    "message": format!("Path conversion failed: {}", conv_err.message),
                    "type": "invalid_request_error",
                    "code": conv_err.code
                }
            });

            return Ok(hyper::Response::builder()
                .status(hyper::StatusCode::BAD_GATEWAY)
                .header("content-type", "application/json")
                .header("X-Conversion-Status", "failed")
                .header("X-Conversion-Phase", "path")
                .header("X-Conversion-Error", conv_err.code)
                .body(full(error_body.to_string()))
                .unwrap());
        }
    };

    // Step 3: Extract model for provider selection
    let model = extract_model(&converted_body);

    // Step 4: Provider selection (filtered by target API type)
    let providers = state.provider_manager
        .select_for_target(target, if model != "unknown" { Some(&model) } else { None })
        .ok_or_else(|| {
            tracing::error!(target = ?target, model = %model, "No matching provider for target API type");
            crate::error::ProxyError::AllProvidersFailed(vec![crate::error::ProxyError::ProviderError(
                "no_matching_provider_for_target_api".to_string()
            )])
        })?;

    if providers.is_empty() {
        let error_body = serde_json::json!({
            "error": {
                "message": format!("No provider available for target API type: {:?}", target),
                "type": "service_unavailable_error"
            }
        });

        return Ok(hyper::Response::builder()
            .status(hyper::StatusCode::BAD_GATEWAY)
            .header("content-type", "application/json")
            .body(full(error_body.to_string()))
            .unwrap());
    }

    // Select first available provider (simplified selection for P4)
    let (provider_name, provider) = providers.first().unwrap();

    // Step 5: Detect if inbound request wants streaming
    let sse_from_headers = is_sse_from_headers(&headers);
    let sse_from_body = crate::stream::is_streaming_body(&converted_body);
    let inbound_is_sse = sse_from_headers || sse_from_body;
    tracing::debug!(
        request_id = %request_id,
        sse_from_headers = sse_from_headers,
        sse_from_body = sse_from_body,
        inbound_is_sse = inbound_is_sse,
        "SSE detection for conversion"
    );

    // Step 6: Attempt request with correct streaming flag
    let request_timeout_secs = {
        let config = state.config.read().unwrap();
        config.server.request_timeout_secs
    };

    let (upstream_resp, _response_bytes, _ttfb_ms, _token_usage, _upstream_url, _sse_total_bytes, _sse_body_buffer, _upstream_error_body, _upstream_response_body) = attempt_request(
        &state.http_client,
        provider_name,
        provider,
        &target_path,
        &method,
        &headers,
        &converted_body,
        &state,
        inbound_is_sse, // Pass detected streaming flag
        request_timeout_secs,
    ).await.map_err(|e| {
        tracing::error!(error = ?e, "Upstream request failed");
        e
    })?;

    // Step 7: Check if upstream response is streaming
    let upstream_status = upstream_resp.status();
    let upstream_is_sse = is_sse_from_headers(&upstream_resp.headers())
        || is_sse_content_type(&upstream_resp.headers());
    tracing::debug!(request_id = %request_id, upstream_status = %upstream_status, upstream_is_sse = upstream_is_sse, "Step 7: Upstream response status");

    // For Responses→Chat conversion with SSE upstream: buffer the stream,
    // extract the final Chat completion JSON, convert it, and return as
    // a Responses API SSE event stream.
    if upstream_is_sse && source == crate::converter::ApiType::OpenAIResponses {
        tracing::info!(request_id = %request_id, "Buffering upstream SSE for Responses→Chat conversion");

        // Check for error response before buffering
        if !upstream_status.is_success() {
            let error_data = upstream_resp.collect().await
                .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
                .to_bytes();
            let error_str = String::from_utf8_lossy(&error_data);
            tracing::warn!(request_id = %request_id, status = %upstream_status, body = %error_str, "Upstream error in SSE response");

            // Emit response.failed event
            let failed_event = serde_json::json!({
                "type": "response.failed",
                "response": {
                    "id": format!("resp_{}", request_id),
                    "object": "response",
                    "status": "failed",
                    "error": {
                        "type": "upstream_error",
                        "message": format!("Upstream returned {}: {}", upstream_status, error_str)
                    }
                }
            });
            let sse = format!("event: response.failed\ndata: {}\n\n", failed_event);
            let sse_bytes = bytes::Bytes::from(sse);
            let stream = futures_util::stream::once(async move {
                Ok::<_, hyper::Error>(hyper::body::Frame::data(sse_bytes))
            });
            let stream_body = http_body_util::StreamBody::new(stream);
            return Ok(hyper::Response::builder()
                .status(upstream_status)
                .header("content-type", "text/event-stream")
                .header("cache-control", "no-cache")
                .body(BoxBody::new(stream_body))
                .unwrap());
        }

        let sse_data = upstream_resp.collect().await
            .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
            .to_bytes();
        let sse_str = String::from_utf8_lossy(&sse_data);

        // Parse SSE chunks: accumulate content from delta chunks,
        // find the final chunk with finish_reason, and build a complete response.
        let mut accumulated_content = String::new();
        let mut accumulated_reasoning = String::new();
        let mut response_id: Option<String> = None;
        let mut model: Option<String> = None;
        let mut created: Option<u64> = None;
        let mut finish_reason: Option<String> = None;
        let mut usage: Option<serde_json::Value> = None;

        for line in sse_str.lines() {
            let line = line.trim();
            if let Some(json_str) = line.strip_prefix("data:") {
                let json_str = json_str.trim();
                if json_str == "[DONE]" { continue; }
                if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if response_id.is_none() {
                        response_id = chunk.get("id").and_then(|v| v.as_str()).map(String::from);
                    }
                    if model.is_none() {
                        model = chunk.get("model").and_then(|v| v.as_str()).map(String::from);
                    }
                    if created.is_none() {
                        created = chunk.get("created").and_then(|v| v.as_u64());
                    }
                    if let Some(u) = chunk.get("usage") {
                        usage = Some(u.clone());
                    }
                    if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                        for choice in choices {
                            // Accumulate content from delta
                            if let Some(delta) = choice.get("delta") {
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                    accumulated_content.push_str(content);
                                }
                                if let Some(reasoning) = delta.get("reasoning_content").and_then(|r| r.as_str()) {
                                    accumulated_reasoning.push_str(reasoning);
                                }
                            }
                            // Also check message.content for non-streaming chunks
                            if let Some(message) = choice.get("message") {
                                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                                    accumulated_content.push_str(content);
                                }
                                if let Some(reasoning) = message.get("reasoning_content").and_then(|r| r.as_str()) {
                                    accumulated_reasoning.push_str(reasoning);
                                }
                            }
                            // Track finish_reason
                            if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                                finish_reason = Some(fr.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Build a complete Chat Completion response from accumulated data
        let complete_response = serde_json::json!({
            "id": response_id.unwrap_or_else(|| "chatcmpl-unknown".to_string()),
            "object": "chat.completion",
            "created": created.unwrap_or(0),
            "model": model.unwrap_or_else(|| "unknown".to_string()),
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": if accumulated_content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(accumulated_content) },
                    "reasoning_content": if accumulated_reasoning.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(accumulated_reasoning) }
                },
                "finish_reason": finish_reason.as_deref().unwrap_or("stop")
            }],
            "usage": usage.map(|u| {
                let prompt = u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let completion = u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                serde_json::json!({
                    "prompt_tokens": prompt,
                    "completion_tokens": completion,
                    "total_tokens": prompt + completion
                })
            }).unwrap_or_else(|| serde_json::json!({"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}))
        });

        // Convert the complete Chat response to Responses format
        let chunk_bytes = bytes::Bytes::from(serde_json::to_vec(&complete_response).unwrap());
        let converted = {
            let converter = &crate::converter::OpenAIResponsesToChatConverter;
            converter.convert_response(&chunk_bytes, target, source, false)
        };

        let converted = match converted {
            Ok(resp) => resp,
            Err(conv_err) => {
                tracing::warn!(request_id = %request_id, error = ?conv_err, "SSE chunk conversion failed");
                return Err(crate::error::ProxyError::HttpError(conv_err.to_string()));
            }
        };

        // Build Responses API SSE event stream
        let resp: serde_json::Value = serde_json::from_slice(&converted).unwrap_or_default();

        let mut sse = String::new();
        // response.created and response.in_progress wrap the response in a "response" field
        let created_event = serde_json::json!({"type": "response.created", "response": resp});
        let in_progress_event = serde_json::json!({"type": "response.in_progress", "response": &resp});
        sse.push_str(&format!("event: response.created\ndata: {}\n\n", created_event));
        sse.push_str(&format!("event: response.in_progress\ndata: {}\n\n", in_progress_event));

        if let Some(output) = resp.get("output").and_then(|o| o.as_array()) {
            for (idx, item) in output.iter().enumerate() {
                // output_item.added wraps item in {type, output_index, item}
                let added_event = serde_json::json!({
                    "type": "response.output_item.added",
                    "output_index": idx,
                    "item": item
                });
                sse.push_str(&format!("event: response.output_item.added\ndata: {}\n\n", added_event));

                if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                    for (part_idx, part) in content.iter().enumerate() {
                        let part_added = serde_json::json!({
                            "type": "response.content_part.added",
                            "output_index": idx,
                            "content_index": part_idx,
                            "part": part
                        });
                        sse.push_str(&format!("event: response.content_part.added\ndata: {}\n\n", part_added));

                        if part.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                            let text = part.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            let delta = serde_json::json!({
                                "type": "response.output_text.delta",
                                "output_index": idx,
                                "content_index": part_idx,
                                "delta": text
                            });
                            sse.push_str(&format!("event: response.output_text.delta\ndata: {}\n\n", delta));
                            let done = serde_json::json!({
                                "type": "response.output_text.done",
                                "output_index": idx,
                                "content_index": part_idx,
                                "text": text
                            });
                            sse.push_str(&format!("event: response.output_text.done\ndata: {}\n\n", done));
                        }

                        let part_done = serde_json::json!({
                            "type": "response.content_part.done",
                            "output_index": idx,
                            "content_index": part_idx,
                            "part": part
                        });
                        sse.push_str(&format!("event: response.content_part.done\ndata: {}\n\n", part_done));
                    }
                }

                let item_done = serde_json::json!({
                    "type": "response.output_item.done",
                    "output_index": idx,
                    "item": item
                });
                sse.push_str(&format!("event: response.output_item.done\ndata: {}\n\n", item_done));
            }
        }

        let completed_event = serde_json::json!({"type": "response.completed", "response": resp});
        sse.push_str(&format!("event: response.completed\ndata: {}\n\n", completed_event));

        let sse_bytes = bytes::Bytes::from(sse);
        let stream = futures_util::stream::once(async move {
            Ok::<_, hyper::Error>(hyper::body::Frame::data(sse_bytes))
        });
        let stream_body = http_body_util::StreamBody::new(stream);
        return Ok(hyper::Response::builder()
            .status(upstream_status)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("X-Conversion-Status", "success")
            .body(BoxBody::new(stream_body))
            .unwrap());
    }

    if upstream_is_sse {
        // Guard: StreamConverter only supports OpenAIChat <-> Anthropic.
        // Responses→Chat SSE is handled by the buffer path above.
        if source == crate::converter::ApiType::OpenAIResponses || target == crate::converter::ApiType::OpenAIResponses {
            tracing::error!(source = ?source, target = ?target, "SSE streaming not supported for Responses API conversion direction");
            return Err(crate::error::ProxyError::HttpError(
                "SSE streaming not supported for this conversion direction".to_string()
            ));
        }

        // True streaming conversion using ConversionStreamBody
        tracing::info!(source = ?source, target = ?target, "Converting streaming response");

        let stream_converter = std::sync::Arc::new(std::sync::Mutex::new(
            crate::converter::stream::StreamConverter::new(target, source)
        ));

        // Extract the upstream body for streaming
        let (mut parts, upstream_body) = upstream_resp.into_parts();

        // Add conversion status headers to the upstream response
        parts.headers.insert(
            hyper::header::HeaderName::from_static("x-conversion-status"),
            hyper::header::HeaderValue::from_static("success"),
        );
        parts.headers.insert(
            hyper::header::HeaderName::from_static("x-conversion-source"),
            source.to_string().parse().unwrap(),
        );
        parts.headers.insert(
            hyper::header::HeaderName::from_static("x-conversion-target"),
            target.to_string().parse().unwrap(),
        );

        // Create streaming body that converts chunks on-the-fly
        let conversion_body = ConversionStreamBody::new(upstream_body, stream_converter);

        return Ok(hyper::Response::from_parts(parts, http_body_util::BodyExt::boxed(conversion_body)));
    }

    // Step 7: Read upstream body
    let upstream_body_bytes = upstream_resp.collect().await
        .map_err(|e| crate::error::ProxyError::HttpError(e.to_string()))?
        .to_bytes();

    tracing::debug!(
        request_id = %request_id,
        upstream_status = %upstream_status,
        upstream_body = %String::from_utf8_lossy(&upstream_body_bytes),
        "Upstream response"
    );

    // Step 8: Select response converter based on target API type (upstream format) and convert with telemetry
    let converted_response = match target {
        crate::converter::ApiType::Anthropic => {
            // target=Anthropic means upstream response is in Anthropic format.
            // We need to convert Anthropic → source (OpenAIChat).
            // convert_response(body, source_format, target_format, is_stream)
            //   where source_format = format of the body passed in (upstream = target)
            //         target_format = format to convert to (client expects = source)
            let converter = &crate::converter::AnthropicToOpenAIConverter;
            if let Some(ref ctx) = telemetry_ctx {
                converter.convert_response_with_ctx(&upstream_body_bytes, ctx.as_ref())
            } else {
                converter.convert_response(&upstream_body_bytes, target, source, false)
            }
        }
        crate::converter::ApiType::OpenAIChat => {
            // target=OpenAIChat means upstream response is in OpenAI format.
            if source == crate::converter::ApiType::OpenAIResponses {
                // /r2c/: upstream Chat → Responses for Codex
                let converter = &crate::converter::OpenAIResponsesToChatConverter;
                converter.convert_response(&upstream_body_bytes, target, source, false)
            } else {
                // We need to convert OpenAIChat → source (Anthropic).
                let converter = &crate::converter::OpenAIChatToAnthropicConverter;
                if let Some(ref ctx) = telemetry_ctx {
                    converter.convert_response_with_ctx(&upstream_body_bytes, ctx.as_ref())
                } else {
                    converter.convert_response(&upstream_body_bytes, target, source, false)
                }
            }
        }
        _ => {
            tracing::error!(target = ?target, "Unsupported target API type for response conversion");
            // Pass through upstream body without conversion
            let body_str = std::str::from_utf8(&upstream_body_bytes).unwrap_or_else(|_| "[non-utf8]");
            return Ok(hyper::Response::builder()
                .status(upstream_status)
                .header("content-type", "application/json")
                .header("X-Conversion-Status", "failed")
                .header("X-Conversion-Phase", "response")
                .header("X-Conversion-Error", "unsupported_target_api")
                .body(full(body_str.to_string()))
                .unwrap());
        }
    };

    let converted_response = match converted_response {
        Ok(resp) => {
            tracing::debug!(request_id = %request_id, "Response conversion succeeded, {} bytes", resp.len());
            resp
        }
        Err(conv_err) => {
            // Check provider config for conversion fallback
            let provider_config = provider.config.read().unwrap();
            let enable_fallback = provider_config.enable_conversion_fallback;
            drop(provider_config);

            // Report error to telemetry if available
            if let Some(ref ctx) = telemetry_ctx {
                ctx.as_ref().report_error(&conv_err);
            }

            // Log detailed error information
            tracing::warn!(
                request_id = %request_id,
                error_code = %conv_err.code,
                field_path = ?conv_err.field_path,
                error = ?conv_err,
                "Response conversion failed"
            );

            // If fallback is disabled, return 502 error
            if !enable_fallback {
                return Err(crate::error::ProxyError::HttpError(conv_err.to_string()));
            }

            // Fallback: pass through upstream body with degradation headers
            let body_str = std::str::from_utf8(&upstream_body_bytes).unwrap_or_else(|_| "[non-utf8]");
            return Ok(hyper::Response::builder()
                .status(upstream_status)
                .header("content-type", "application/json")
                .header("X-Conversion-Status", "failed")
                .header("X-Conversion-Phase", "response")
                .header("X-Conversion-Error", conv_err.code)
                .body(full(body_str.to_string()))
                .unwrap());
        }
    };

    // Step 9: Return converted response
    tracing::debug!(request_id = %request_id, source = ?source, "Step 9: Returning converted response");
    Ok(hyper::Response::builder()
        .status(upstream_status)
        .header("content-type", "application/json")
        .header("X-Conversion-Status", "success")
        .body(full(converted_response))
        .unwrap())
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

/// Check if response Content-Type indicates SSE
fn is_sse_content_type(headers: &hyper::HeaderMap) -> bool {
    if let Some(ct) = headers.get(hyper::header::CONTENT_TYPE) {
        if let Ok(ct_str) = ct.to_str() {
            return ct_str.contains("text/event-stream");
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
    for (name, value) in rewritten_headers.iter() {
        tracing::debug!(header = %name, value = ?value, "Upstream request header");
    }

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

        let body = http_body_util::BodyExt::boxed(StreamBody::new(stream));

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
    pub telemetry: Arc<crate::converter::telemetry::InMemoryTelemetry>, // TODO: Wire into conversion handlers
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_request_id_is_unique() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_request_id_format() {
        let id = generate_request_id();
        // Request IDs should be non-empty and reasonable length
        assert!(!id.is_empty());
        assert!(id.len() <= 64);
    }

    #[test]
    fn test_is_sse_from_headers_with_event_stream() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert(hyper::header::ACCEPT, "text/event-stream".parse().unwrap());
        assert!(is_sse_from_headers(&headers));
    }

    #[test]
    fn test_is_sse_from_headers_without_event_stream() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert(hyper::header::ACCEPT, "application/json".parse().unwrap());
        assert!(!is_sse_from_headers(&headers));
    }

    #[test]
    fn test_is_sse_from_headers_missing_accept() {
        let headers = hyper::HeaderMap::new();
        assert!(!is_sse_from_headers(&headers));
    }
}