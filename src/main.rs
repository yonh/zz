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
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;

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
    let mut config = config::Config::load(&args.config)?;

    // Override with CLI args
    if let Some(listen) = args.listen {
        config.server.listen = listen;
    }
    if let Some(log_level) = args.log_level {
        config.server.log_level = log_level;
    }

    // Initialize logging
    logging::init_logging(&config.server.log_level)?;

    tracing::info!(listen = %config.server.listen, "Starting ZZ proxy");

    // Create shared state
    let provider_manager = Arc::new(provider::ProviderManager::new(&config));
    let router = Arc::new(router::Router::new(&config.routing.strategy));
    let config = Arc::new(config);

    let state = proxy::AppState {
        provider_manager,
        router,
        config,
    };

    // Create server
    let addr: std::net::SocketAddr = state.config.server.listen.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = hyper_util::rt::TokioIo::new(stream);
        let state = state.clone();

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req| {
                let state = state.clone();
                async move {
                    // Check admin endpoints first
                    if let Some(resp) = admin::handle_admin_request(&req) {
                        return Ok::<_, hyper::Error>(resp);
                    }

                    // Handle proxy request
                    match proxy::proxy_handler(req, state).await {
                        Ok(resp) => Ok(resp),
                        Err(e) => {
                            tracing::error!(error = ?e, "Proxy error");
                            Ok(hyper::Response::builder()
                                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Full::from(format!("Proxy error: {}", e)))
                                .unwrap())
                        }
                    }
                }
            });

            if let Err(err) = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                tracing::error!(error = ?err, "Connection error");
            }
        });
    }
}