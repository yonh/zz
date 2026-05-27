//! Mock upstream HTTP server + zz test harness for integration tests.
//!
//! Usage in test file:
//! ```rust,no_run
//! mod common;
//! let (mock, upstream_url) = common::MockServer::start(...).await;
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Describes how the mock upstream responds to a request
pub enum ChatHandler {
    /// Returns a fixed response
    Fixed {
        status: u16,
        body: &'static str,
        delay_ms: u64,
    },
    /// Validates the request matches expectations and responds
    Validating {
        expected_path: &'static str,
        expected_method: &'static str,
        expected_body_contains: &'static [&'static str],
        status: u16,
        body: &'static str,
    },
}

/// A mock server that accepts Chat Completion API requests and returns controlled responses
pub struct MockServer {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl MockServer {
    /// Start a mock server. Returns (MockServer, base_url like "http://127.0.0.1:PORT")
    pub async fn start(handler: ChatHandler) -> (Self, String) {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = Arc::clone(&stopped);

        let handler = Arc::new(handler);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        let (stream, _) = accept.unwrap();
                        let io = hyper_util::rt::TokioIo::new(stream);
                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            let svc = hyper::service::service_fn(move |req| {
                                let handler = Arc::clone(&handler);
                                async move {
                                    let (parts, body) = req.into_parts();
                                    let body_bytes = http_body_util::BodyExt::collect(body).await
                                        .map(|b| b.to_bytes()).unwrap_or_default();

                                    match &*handler {
                                        ChatHandler::Fixed { status, body, delay_ms } => {
                                            if *delay_ms > 0 {
                                                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                                            }
                                            Ok::<_, hyper::Error>(json_response(*status, body))
                                        }
                                        ChatHandler::Validating { expected_path, expected_method, expected_body_contains, status, body } => {
                                            assert_eq!(parts.uri.path(), *expected_path, "Path mismatch");
                                            assert_eq!(parts.method.as_str(), *expected_method, "Method mismatch");
                                            let body_str = String::from_utf8_lossy(&body_bytes);
                                            for expected in *expected_body_contains {
                                                assert!(body_str.contains(expected),
                                                    "Body should contain '{}', got: {}", expected, body_str);
                                            }
                                            Ok(json_response(*status, body))
                                        }
                                    }
                                }
                            });
                            let builder = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
                            let _ = builder.serve_connection_with_upgrades(io, svc).await;
                        });
                    }
                    _ = &mut shutdown_rx => {
                        stopped_clone.store(true, Ordering::Relaxed);
                        break;
                    }
                    else => break,
                }
            }
        });

        let base_url = format!("http://{}", addr);
        (MockServer { shutdown_tx: Some(shutdown_tx) }, base_url)
    }

    /// Stop the mock server
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

fn json_response(status: u16, body: &str) -> hyper::Response<http_body_util::Full<hyper::body::Bytes>> {
    hyper::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(hyper::body::Bytes::from(body.to_string())))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Predefined Chat Completion response helpers
// ---------------------------------------------------------------------------

/// A simple text response
pub fn chat_ok_response(text: &str) -> String {
    format!(r#"{{"id":"chatcmpl-mock","object":"chat.completion","created":1748332800,"model":"gpt-4o-mini","choices":[{{"index":0,"message":{{"role":"assistant","content":"{text}"}},"finish_reason":"stop"}}],"usage":{{"prompt_tokens":10,"completion_tokens":5}}}}"#)
}

/// A tool call response
pub fn chat_tool_call_response() -> String {
    r#"{"id":"chatcmpl-mock","object":"chat.completion","created":1748332800,"model":"gpt-4o-mini","choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Tokyo\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":50,"completion_tokens":10}}"#.to_string()
}

/// Create a zz config TOML string pointing at the given upstream URL
pub fn zz_config(upstream_url: &str) -> String {
    format!(r#"
[server]
listen = "127.0.0.1:0"
request_timeout_secs = 30
log_level = "error"

[admin]
enabled = false

[routing]
strategy = "failover"
max_retries = 1

[health]
failure_threshold = 10
recovery_secs = 10
cooldown_secs = 5

[[providers]]
name = "test-upstream"
base_url = "{upstream_url}"
api_key = "sk-test-key"
priority = 1
models = ["*"]
api_type = "openai-chat"
"#)
}

/// Send a JSON POST request and return (status, body_bytes)
pub async fn post_json(url: &str, body: &str) -> (hyper::StatusCode, hyper::body::Bytes) {
    let client = hyper_util::client::legacy::Client::builder(
        hyper_util::rt::TokioExecutor::new()
    ).build_http();

    let req = hyper::Request::post(url)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(hyper::body::Bytes::from(body.to_string())))
        .unwrap();

    let resp = client.request(req).await.unwrap();
    let status = resp.status();
    let body_bytes = http_body_util::BodyExt::collect(resp.into_body()).await
        .unwrap().to_bytes();
    (status, body_bytes)
}