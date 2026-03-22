mod config;
mod provider;
mod router;
mod rewriter;
mod error;
mod stream;
mod proxy;
mod admin;
mod logging;
mod cors;
mod stats;
mod admin_api;
mod ws;

use clap::Parser;
use std::sync::Arc;
use http_body_util::BodyExt;
use hyper::body::Bytes;
use std::time::Instant;

type ResponseBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> ResponseBody {
    http_body_util::Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[derive(Parser, Debug)]
#[command(name = "zz")]
#[command(about = "LLM API Reverse Proxy with Auto-Failover")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short, long)]
    listen: Option<String>,

    #[arg(long)]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load config
    let mut cfg = config::Config::load(&args.config)?;

    // Override with CLI args
    if let Some(listen) = args.listen {
        cfg.server.listen = listen;
    }
    if let Some(log_level) = args.log_level {
        cfg.server.log_level = log_level;
    }

    // Initialize logging
    logging::init_logging(&cfg.server.log_level)?;

    tracing::info!(listen = %cfg.server.listen, "Starting ZZ proxy");

    // Create shared state
    let provider_manager = Arc::new(provider::ProviderManager::new(&cfg));
    let router = Arc::new(router::Router::new(&cfg.routing.strategy));
    let log_buffer = Arc::new(stats::RequestLogBuffer::new(10000));
    let ws_broadcaster = Arc::new(ws::WsBroadcaster::new());
    let rpm_counter = Arc::new(stats::RpmCounter::new());
    let model_rules = Arc::new(std::sync::RwLock::new(
        cfg.routing.rules.iter().enumerate().map(|(i, r)| router::ModelRule {
            id: format!("rule_{}", i + 1),
            pattern: r.pattern.clone(),
            target_provider: r.target_provider.clone(),
        }).collect::<Vec<_>>()
    ));
    let start_time = Instant::now();

    let state = proxy::AppState {
        provider_manager,
        router,
        config: Arc::new(std::sync::RwLock::new(cfg)),
        config_path: args.config.clone(),
        start_time,
        log_buffer,
        ws_broadcaster: ws_broadcaster.clone(),
        model_rules,
        rpm_counter,
    };

    // Create server
    let addr: std::net::SocketAddr = {
        let config = state.config.read().unwrap();
        config.server.listen.parse()?
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on {}", addr);

    // Graceful shutdown setup
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Spawn periodic stats broadcaster (every 5 seconds)
    {
        let state_clone = state.clone();
        let mut shutdown_rx_clone = shutdown_rx.resubscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let (total_requests, total_errors) = state_clone.provider_manager.get_total_stats();
                        let all_stats = state_clone.provider_manager.get_all_stats();
                        let active = all_stats.iter().filter(|s| s.enabled).count();
                        let healthy = all_stats.iter().filter(|s| s.state == "healthy" && s.enabled).count();
                        let total = all_stats.len();

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
                    _ = shutdown_rx_clone.recv() => {
                        tracing::debug!("Stats broadcaster shutting down");
                        break;
                    }
                }
            }
        });
    }

    // Handle Ctrl+C
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Shutdown signal received");
        let _ = shutdown_tx_clone.send(());
    });

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (stream, _) = accept_result?;
                let io = hyper_util::rt::TokioIo::new(stream);
                let state = state.clone();
                let mut shutdown_rx_clone = shutdown_rx.resubscribe();

                tokio::spawn(async move {
                    let service = hyper::service::service_fn(move |req| {
                        let state = state.clone();
                        async move {
                            let path = req.uri().path().to_string();

                            // Handle WebSocket upgrade (/zz/ws)
                            if path == "/zz/ws" {
                                let resp = ws::handle_ws_request(req, state).await;
                                return Ok::<_, hyper::Error>(resp.map(|b| b.map_err(|never| match never {}).boxed()));
                            }

                            // Handle API requests (/zz/api/*)
                            if path.starts_with("/zz/api/") {
                                if let Some(resp) = admin_api::handle_api_request(req, state).await {
                                    return Ok::<_, hyper::Error>(resp);
                                }
                                // If admin_api didn't handle it, return 404
                                return Ok::<_, hyper::Error>(
                                    hyper::Response::builder()
                                        .status(hyper::StatusCode::NOT_FOUND)
                                        .body(full("Not found"))
                                        .unwrap()
                                );
                            }

                            // Handle legacy admin endpoints (/zz/health, /zz/stats, /zz/reload)
                            if path.starts_with("/zz/health") || path.starts_with("/zz/stats") || path.starts_with("/zz/reload") {
                                if let Some(resp) = admin::handle_admin_request(&req, Some(&state)) {
                                    return Ok::<_, hyper::Error>(resp);
                                }
                            }

                            // Handle proxy request
                            match proxy::proxy_handler(req, state).await {
                                Ok(resp) => Ok(resp),
                                Err(e) => {
                                    tracing::error!(error = ?e, "Proxy error");
                                    Ok(hyper::Response::builder()
                                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(full(format!("Proxy error: {}", e)))
                                        .unwrap())
                                }
                            }
                        }
                    });

                    let builder = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
                    let conn = builder.serve_connection_with_upgrades(io, service);

                    tokio::select! {
                        result = conn => {
                            if let Err(err) = result {
                                tracing::error!(error = ?err, "Connection error");
                            }
                        }
                        _ = shutdown_rx_clone.recv() => {
                            tracing::debug!("Connection shutting down");
                        }
                    }
                });
            }
            _ = shutdown_rx.recv() => {
                tracing::info!("Shutting down server");
                break;
            }
        }
    }

    tracing::info!("Server stopped");
    Ok(())
}