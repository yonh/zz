mod config;
mod provider;
mod router;
mod rewriter;
mod error;
mod stream;
mod proxy;
mod admin;
mod logging;

use clap::Parser;
use std::sync::Arc;
use http_body_util::BodyExt;
use hyper::body::Bytes;

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

    #[arg(short, long)]
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
    let config = Arc::new(cfg);

    let state = proxy::AppState {
        provider_manager,
        router,
        config,
    };

    // Create server
    let addr: std::net::SocketAddr = state.config.server.listen.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on {}", addr);

    // Graceful shutdown setup
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

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
                            // Check admin endpoints first
                            if let Some(resp) = admin::handle_admin_request(&req, Some(&state)) {
                                return Ok::<_, hyper::Error>(resp);
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
                    let conn = builder.serve_connection(io, service);

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