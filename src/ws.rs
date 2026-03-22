use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use futures_util::{StreamExt, SinkExt};

// Re-export LogEntry from stats
pub use crate::stats::LogEntry;

/// WebSocket event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WsEvent {
    Log(LogEntry),
    ProviderState(ProviderStateEvent),
    Stats(StatsSnapshot),
}

/// Provider state change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStateEvent {
    pub name: String,
    pub status: String,
    pub cooldown_until: Option<String>,
}

/// System statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub total_requests: u64,
    pub total_errors: u64,
    pub requests_per_minute: f64,
    pub active_providers: usize,
    pub healthy_providers: usize,
    pub total_providers: usize,
    pub uptime_secs: u64,
}

/// WebSocket broadcaster
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, event: WsEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = self.tx.send(json);
        }
    }

    pub fn broadcast_log(&self, entry: LogEntry) {
        self.broadcast(WsEvent::Log(entry));
    }

    pub fn broadcast_provider_state(
        &self,
        name: &str,
        status: &str,
        cooldown_until: Option<String>,
    ) {
        self.broadcast(WsEvent::ProviderState(ProviderStateEvent {
            name: name.to_string(),
            status: status.to_string(),
            cooldown_until,
        }));
    }

    pub fn broadcast_stats(&self, snapshot: StatsSnapshot) {
        self.broadcast(WsEvent::Stats(snapshot));
    }
}

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WsBroadcaster {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

/// Handle WebSocket connection after HTTP upgrade.
pub async fn handle_ws_connection(
    upgraded: hyper::upgrade::Upgraded,
    broadcaster: std::sync::Arc<WsBroadcaster>,
) {
    use tokio_tungstenite::tungstenite::protocol::Message;
    use hyper_util::rt::TokioIo;

    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        TokioIo::new(upgraded),
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

    // Read from client (handle close, ping/pong)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {
                    // Pong is handled automatically by tungstenite
                }
                Message::Text(_text) => {
                    // Could handle subscribe filter here
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

/// Handle WebSocket upgrade from HTTP request.
pub async fn handle_ws_request(
    mut req: hyper::Request<hyper::body::Incoming>,
    state: crate::proxy::AppState,
) -> hyper::Response<http_body_util::Full<hyper::body::Bytes>> {
    use http_body_util::Full;
    use hyper::body::Bytes;
    use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

    // Extract Sec-WebSocket-Key for the handshake response
    let ws_key = req.headers()
        .get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let accept_key = derive_accept_key(ws_key.as_bytes());

    // IMPORTANT: call upgrade::on(&mut req) *before* returning the response.
    // hyper requires the upgrade future to be registered while the request is
    // still in the connection handler's poll chain — not inside a spawned task.
    let upgrade_fut = hyper::upgrade::on(&mut req);
    let broadcaster = state.ws_broadcaster.clone();

    // Spawn to run the actual WS connection after the upgrade resolves
    tokio::spawn(async move {
        match upgrade_fut.await {
            Ok(upgraded) => {
                handle_ws_connection(upgraded, broadcaster).await;
            }
            Err(e) => {
                tracing::error!(error = ?e, "WebSocket upgrade failed");
            }
        }
    });

    // Return 101 Switching Protocols with proper WebSocket accept header
    hyper::Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "Upgrade")
        .header("Sec-WebSocket-Accept", accept_key)
        .body(Full::new(Bytes::new()))
        .unwrap()
}
