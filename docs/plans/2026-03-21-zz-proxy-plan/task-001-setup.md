# Task 001: Setup Project Dependencies and Structure

## Goal
Initialize Rust project with all required dependencies and module structure.

## Files to Create/Edit

**Modify**:
- `Cargo.toml` - Add dependencies
- `src/main.rs` - Update module declarations

**Create**:
- `src/config.rs`
- `src/provider.rs`
- `src/router.rs`
- `src/rewriter.rs`
- `src/error.rs`
- `src/stream.rs`
- `src/proxy.rs`
- `src/admin.rs`
- `src/logging.rs`

## Implementation Steps

1. Update `Cargo.toml` with dependencies:
   - `tokio` (async runtime)
   - `hyper`, `hyper-util`, `http-body-util` (HTTP server/client)
   - `tokio-util` (stream utilities)
   - `serde`, `serde_derive`, `toml` (config parsing)
   - `tracing`, `tracing-subscriber` (logging)
   - `clap` (CLI)
   - `dashmap` (lock-free shared state)
   - `anyhow` (error handling)
   - `chrono` (timestamps)

2. Create all module files with empty structs and placeholder comments

3. Add module declarations in `src/main.rs`

## Verification

Run:
```bash
cargo check
```

Expected: No compilation errors, all modules recognized.

## Dependencies
- None
