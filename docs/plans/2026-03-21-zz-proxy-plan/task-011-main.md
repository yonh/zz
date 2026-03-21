# Task 011: Main Entry Point - Server Startup

## Goal
Implement main entry point with CLI argument parsing and HTTP server startup.

## BDD Scenarios

```gherkin
Scenario: Start server on configured address
  Given config.toml has server.listen = "127.0.0.1:9090"
  When zz is started with default config path
  Then server listens on 127.0.0.1:9090
  And responds to HTTP requests

Scenario: Override config path via CLI
  Given CLI argument --config /tmp/custom.toml
  When zz is started
  Then loads config from /tmp/custom.toml instead of default

Scenario: Override listen address via CLI
  Given CLI argument --listen 0.0.0.0:8080
  When zz is started
  Then server listens on 0.0.0.0:8080 (overrides config)

Scenario: Show help message
  Given CLI argument --help
  When zz is started
  Then prints usage information
  And exits with code 0

Scenario: Route admin and proxy paths
  Given request path = /zz/health
  When request arrives at server
  Then handled by admin router
  Given request path = /v1/chat/completions
  When request arrives at server
  Then handled by proxy router

Scenario: Graceful shutdown on SIGINT
  Given server is running
  When user presses Ctrl+C
  Then server stops accepting new connections
  And waits for in-flight requests to complete
  And exits cleanly
```

## Files to Create/Edit

**Modify**:
- `src/main.rs` - Complete implementation

## Implementation Steps

1. Add CLI argument parsing with clap:
   ```rust
   #[derive(Parser)]
   struct Args {
       #[arg(short, long, default_value = "config.toml")]
       config: String,

       #[arg(short, long)]
       listen: Option<String>,

       #[arg(short, long, default_value = "info")]
       log_level: Option<String>,
   }
   ```

2. Implement main function:
   - Parse CLI args
   - Load config (override with CLI args if provided)
   - Initialize logging
   - Create ProviderManager from config
   - Create Router with strategy from config
   - Create shared state (Arc<AppState>)

3. Create HTTP server:
   - Use `hyper_util::server::conn::auto::Builder` for HTTP/1.1 + HTTP/2
   - Use `tokio::net::TcpListener` to bind address
   - Implement service fn that:
     - Checks admin path first (admin::handle_admin_request)
     - Falls back to proxy::proxy_handler
   - Spawn server task with graceful shutdown support

4. Implement request handler:
   ```rust
   async fn handle_request(req: Request<Incoming>, state: AppState) -> Result<Response<...>> {
       if let Some(resp) = admin::handle_request(&req) {
           return Ok(resp);
       }
       proxy::proxy_handler(req, state).await
   }
   ```

5. Add graceful shutdown:
   - Listen for SIGINT/SIGTERM
   - Stop accepting new connections
   - Wait for active requests to complete
   - Shutdown tokio runtime

## Verification

Run:
```bash
cargo run -- --help
cargo run 2>&1 | grep "Listening"
```

Expected:
- Help message displays correctly
- Server starts and binds to configured address
- No panic on startup

## Dependencies
- Task 008 (Proxy handler)
- Task 009 (Admin endpoints)
- Task 010 (Logging initialization)
