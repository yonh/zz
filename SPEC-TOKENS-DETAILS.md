# ZZ - Token Statistics Implementation Details

## Document Purpose

This document provides detailed implementation specifications for edge cases, boundary conditions, and complex scenarios in the token statistics system. It complements SPEC-TOKENS.md with deeper technical guidance.

---

## 1. Streaming SSE Token Extraction (Critical)

### 1.1 Problem Statement

For streaming (SSE) responses, the token usage is typically included in the **final chunk**, not the first response. This creates challenges:

1. We stream response directly to client without buffering
2. Token usage arrives only after streaming completes
3. Need to update log entry retroactively

### 1.2 Provider-Specific Behavior

| Provider | Usage Location | Format |
|----------|----------------|--------|
| OpenAI | Final chunk | `data: {"usage": {...}}` |
| DashScope (Ali) | First chunk (sometimes) or final | `data: {"output": {"usage": {...}}}` |
| Zhipu GLM | Final chunk | `data: {"usage": {...}}` |
| Anthropic | Final chunk (message_stop) | `data: {"type": "message_delta", "usage": {...}}` |

### 1.3 Recommended Solution: Dual-Path Extraction

```rust
// src/token_extractor.rs

/// SSE chunk classifier
pub enum SseChunkType {
    /// Contains usage information
    Usage(TokenUsage),
    /// Regular content chunk
    Content,
    /// Stream ended
    Done,
    /// Unparseable
    Unknown,
}

/// Extract usage from a single SSE chunk (called during streaming)
pub fn extract_usage_from_chunk(chunk: &str) -> SseChunkType {
    // Skip empty lines
    if chunk.trim().is_empty() {
        return SseChunkType::Unknown;
    }
    
    // Parse SSE format: "data: {...}" or "data: [DONE]"
    if let Some(json_str) = chunk.strip_prefix("data: ") {
        if json_str.trim() == "[DONE]" {
            return SseChunkType::Done;
        }
        
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Standard OpenAI format
            if let Some(usage) = v.get("usage") {
                if let Some(tokens) = parse_usage_object(usage) {
                    return SseChunkType::Usage(tokens);
                }
            }
            
            // DashScope format: usage under "output"
            if let Some(output) = v.get("output") {
                if let Some(usage) = output.get("usage") {
                    if let Some(tokens) = parse_usage_object(usage) {
                        return SseChunkType::Usage(tokens);
                    }
                }
            }
            
            // Anthropic format: type "message_delta"
            if v.get("type").and_then(|t| t.as_str()) == Some("message_delta") {
                if let Some(usage) = v.get("usage") {
                    if let Some(tokens) = parse_usage_object(usage) {
                        return SseChunkType::Usage(tokens);
                    }
                }
            }
        }
    }
    
    SseChunkType::Content
}

fn parse_usage_object(usage: &serde_json::Value) -> Option<TokenUsage> {
    let input = usage.get("prompt_tokens")?.as_u64()? as u32;
    let output = usage.get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))  // Some providers use output_tokens
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as u32;
    
    let total = usage.get("total_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or((input + output) as u64) as u32;
    
    let cached = usage.get("cached_tokens")
        .or_else(|| usage.get("cache_read_tokens"))  // Anthropic format
        .and_then(|t| t.as_u64())
        .map(|t| t as u32);
    
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        cached_tokens: cached,
    })
}
```

### 1.4 Streaming Architecture Change

**Current flow** (in `proxy.rs`):
```
Request → Provider → Stream Response → Client (direct pipe)
```

**New flow with token extraction**:
```
Request → Provider → Stream Response 
                          ↓
                    ┌─────────────────┐
                    │ ChunkProcessor  │ ← Inspect each chunk
                    │  - Forward to   │   for usage
                    │    client       │
                    │  - Extract      │
                    │    usage        │
                    └─────────────────┘
                          ↓
                    Client (still streaming)
                          ↓
                    On stream end:
                    Update log entry with usage
```

**Implementation**:

```rust
// src/stream.rs - NEW streaming processor

use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use bytes::Bytes;
use std::sync::Arc;

/// Wrapper stream that extracts token usage while forwarding
pub struct TokenExtractingStream<S> {
    inner: S,
    chunks_buffer: Vec<String>,
    usage: Option<TokenUsage>,
    on_complete: Arc<dyn Fn(TokenUsage) + Send + Sync>,
    provider_name: String,
}

impl<S> TokenExtractingStream<S>
where
    S: Stream<Item = Result<Bytes, hyper::Error>> + Unpin,
{
    pub fn new(
        inner: S,
        provider_name: String,
        on_complete: Arc<dyn Fn(TokenUsage) + Send + Sync>,
    ) -> Self {
        Self {
            inner,
            chunks_buffer: Vec::with_capacity(50),
            usage: None,
            on_complete,
            provider_name,
        }
    }
}

impl<S> Stream for TokenExtractingStream<S>
where
    S: Stream<Item = Result<Bytes, hyper::Error>> + Unpin,
{
    type Item = Result<Bytes, hyper::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                // Try to extract usage from this chunk
                if let Ok(chunk_str) = std::str::from_utf8(&chunk) {
                    for line in chunk_str.lines() {
                        if line.starts_with("data: ") {
                            match extract_usage_from_chunk(line) {
                                SseChunkType::Usage(usage) => {
                                    self.usage = Some(usage);
                                    tracing::debug!(
                                        provider = %self.provider_name,
                                        input = usage.input_tokens,
                                        output = usage.output_tokens,
                                        "Extracted token usage from SSE stream"
                                    );
                                }
                                SseChunkType::Done => {
                                    // Stream complete, invoke callback
                                    if let Some(usage) = self.usage.take() {
                                        (self.on_complete)(usage);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended, invoke callback if we have usage
                if let Some(usage) = self.usage.take() {
                    (self.on_complete)(usage);
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
```

### 1.5 Retroactive Log Update

```rust
// In proxy.rs, modify the streaming path:

if is_sse {
    // Create callback for token update
    let log_id = request_id.clone();
    let storage = state.storage.clone();
    let pricing = state.pricing.clone();
    let model = model.clone();
    let provider = provider_name.clone();
    
    let on_usage = Arc::new(move |usage: TokenUsage| {
        let cost = pricing.calculate_cost(&model, usage.input_tokens, usage.output_tokens);
        
        // Update log entry in database
        if let Err(e) = storage.update_log_tokens(&log_id, usage, cost) {
            tracing::error!(log_id = %log_id, error = %e, "Failed to update log tokens");
        }
        
        // Update current usage for quota tracking
        if let Err(e) = storage.increment_usage(&provider, usage.total_tokens, cost) {
            tracing::error!(provider = %provider, error = %e, "Failed to update usage");
        }
    });
    
    // Wrap stream with token extraction
    let wrapped_stream = TokenExtractingStream::new(
        response.into_data_stream(),
        provider_name.clone(),
        on_usage,
    );
    
    // Stream to client
    let body = StreamBody::new(wrapped_stream.map_ok(Frame::data)).boxed();
    // ... return response
}
```

### 1.6 Fallback for Missing Usage

If no usage is extracted from SSE stream:

```rust
// After stream completes without usage info
if usage.is_none() {
    tracing::warn!(
        provider = %provider_name,
        request_id = %request_id,
        "No token usage extracted from SSE stream - marking as unknown"
    );
    
    // Store with null token fields (already handled by Option<u32>)
    // Do NOT estimate based on bytes - too inaccurate
}
```

**Do NOT estimate tokens from bytes**:
- Different tokenizers have different ratios
- Chinese text: ~1.5 chars/token
- English text: ~4 chars/token
- Code: highly variable

---

## 2. Provider Response Format Variations

### 2.1 OpenAI Format (Standard)

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "choices": [...],
  "usage": {
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_tokens": 150
  }
}
```

### 2.2 DashScope (Alibaba) Format

**Non-streaming**:
```json
{
  "output": {
    "choices": [...],
    "usage": {
      "prompt_tokens": 100,
      "output_tokens": 50,  // Note: output_tokens not completion_tokens
      "total_tokens": 150
    }
  },
  "request_id": "xxx"
}
```

**Streaming**:
```json
data: {"output": {"choices": [...]}, "usage": {"prompt_tokens": 100, "output_tokens": 50}}
data: [DONE]
```

### 2.3 Zhipu GLM Format

```json
{
  "id": "xxx",
  "choices": [...],
  "usage": {
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_tokens": 150
  }
}
```

### 2.4 Anthropic Format

**Non-streaming**:
```json
{
  "id": "msg_xxx",
  "type": "message",
  "content": [...],
  "usage": {
    "input_tokens": 100,
    "output_tokens": 50,
    "cache_read_tokens": 20,  // Optional: prompt caching
    "cache_creation_tokens": 10  // Optional
  }
}
```

**Streaming**:
```json
data: {"type": "message_start", "message": {...}}
data: {"type": "content_block_delta", ...}
data: {"type": "message_delta", "usage": {"output_tokens": 50}}
data: {"type": "message_stop"}
```

### 2.5 Unified Extraction Logic

```rust
// src/token_extractor.rs

pub fn extract_usage(body: &[u8], provider_hint: Option<&str>) -> Option<TokenUsage> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    
    // Try different paths based on provider
    let usage_paths = match provider_hint {
        Some("dashscope") | Some("ali") => vec!["output.usage", "usage"],
        Some("anthropic") => vec!["usage"],
        Some("zhipu") | Some("glm") => vec!["usage"],
        _ => vec!["usage", "output.usage"],  // Try all
    };
    
    for path in usage_paths {
        if let Some(usage) = get_nested(&v, path) {
            if let Some(tokens) = parse_usage_unified(usage) {
                return Some(tokens);
            }
        }
    }
    
    None
}

fn get_nested<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = v;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn parse_usage_unified(usage: &serde_json::Value) -> Option<TokenUsage> {
    // Try different field names
    let input = usage.get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))  // Anthropic
        .and_then(|t| t.as_u64())? as u32;
    
    let output = usage.get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))  // DashScope, Anthropic
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as u32;
    
    let total = usage.get("total_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or((input + output) as u64) as u32;
    
    // Cache tokens (optional)
    let cached = usage.get("cached_tokens")
        .or_else(|| usage.get("cache_read_tokens"))
        .and_then(|t| t.as_u64())
        .map(|t| t as u32);
    
    Some(TokenUsage { input_tokens: input, output_tokens: output, total_tokens: total, cached_tokens: cached })
}
```

---

## 3. Database Concurrency & Safety

### 3.1 SQLite Configuration

```rust
// src/storage/mod.rs

impl TokenStorage {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path)?;
        
        // Essential for concurrent access
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        conn.pragma_update(None, "synchronous", &"NORMAL")?;
        conn.pragma_update(None, "busy_timeout", &"5000")?;  // 5s wait for lock
        conn.pragma_update(None, "cache_size", &"-64000")?;  // 64MB cache
        
        // For better write performance
        conn.pragma_update(None, "temp_store", &"MEMORY")?;
        
        Ok(Self { conn: Mutex::new(conn), db_path: db_path.to_string() })
    }
}
```

### 3.2 Write Queue Pattern

For high-throughput scenarios, use a write queue:

```rust
// src/storage/write_queue.rs

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

pub struct WriteQueue {
    sender: Sender<WriteOp>,
}

enum WriteOp {
    InsertLog(LogEntry),
    UpdateUsage { provider: String, tokens: u32, cost: f64 },
    Shutdown,
}

impl WriteQueue {
    pub fn start(storage: Arc<TokenStorage>) -> Self {
        let (sender, receiver) = channel::<WriteOp>();
        
        thread::spawn(move || {
            let mut batch = Vec::with_capacity(100);
            let mut last_flush = std::time::Instant::now();
            
            loop {
                // Collect batch with timeout
                match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(WriteOp::InsertLog(entry)) => {
                        batch.push(entry);
                        
                        // Flush if batch full or time elapsed
                        if batch.len() >= 100 || last_flush.elapsed().as_millis() > 1000 {
                            if let Err(e) = storage.insert_logs_batch(&batch) {
                                tracing::error!("Batch insert failed: {}", e);
                            }
                            batch.clear();
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Ok(WriteOp::UpdateUsage { provider, tokens, cost }) => {
                        // Immediate write for quota tracking
                        if let Err(e) = storage.increment_usage(&provider, tokens, cost) {
                            tracing::error!("Usage update failed: {}", e);
                        }
                    }
                    Ok(WriteOp::Shutdown) | Err(_) => break,
                    _ => {}
                }
            }
            
            // Flush remaining on shutdown
            if !batch.is_empty() {
                let _ = storage.insert_logs_batch(&batch);
            }
        });
        
        Self { sender }
    }
    
    pub fn insert_log(&self, entry: LogEntry) {
        let _ = self.sender.send(WriteOp::InsertLog(entry));
    }
}
```

### 3.3 Transaction Strategy

```rust
impl TokenStorage {
    /// Batch insert with transaction
    pub fn insert_logs_batch(&self, entries: &[LogEntry]) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO request_logs (...) VALUES (?1, ?2, ...)"
            ).map_err(|e| e.to_string())?;
            
            for entry in entries {
                stmt.execute(params![
                    entry.id, entry.timestamp, entry.provider, ...
                ]).map_err(|e| e.to_string())?;
            }
        }
        
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
}
```

---

## 4. Quota Reset Edge Cases

### 4.1 Reset Timing

```rust
// src/storage/quota.rs

impl TokenStorage {
    /// Check and reset quotas if needed
    pub fn check_and_reset_quotas(&self, now: chrono::DateTime<chrono::Utc>) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        
        // Get all quotas with their reset days
        let quotas: Vec<QuotaResetInfo> = conn
            .prepare("SELECT provider, reset_day FROM provider_quotas")?
            .query_map([], |row| Ok(QuotaResetInfo {
                provider: row.get(0)?,
                reset_day: row.get(1)?,
            }))?
            .collect::<Result<Vec<_>, _>>()?;
        
        for quota in quotas {
            // Check if today is the reset day
            let should_reset = self.should_reset_now(&conn, &quota, now)?;
            
            if should_reset {
                self.reset_provider_quota(&conn, &quota.provider, now)?;
            }
        }
        
        Ok(())
    }
    
    fn should_reset_now(
        &self,
        conn: &Connection,
        quota: &QuotaResetInfo,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, String> {
        // Get current period start
        let period_start: Option<String> = conn
            .query_row(
                "SELECT period_start FROM current_usage WHERE provider = ?1",
                [&quota.provider],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        
        let Some(period_start) = period_start else {
            return Ok(false);  // No usage yet
        };
        
        let period_start = chrono::DateTime::parse_from_rfc3339(&period_start)
            .map_err(|e| e.to_string())?
            .with_timezone(&chrono::Utc);
        
        // Check if we've passed the reset day in a new month
        let reset_day = quota.reset_day as u32;
        
        // Current day of month
        let current_day = now.day();
        
        // Period's month
        let period_month = period_start.month();
        let current_month = now.month();
        
        // Reset if:
        // 1. We're in a different month, AND
        // 2. Today is >= reset day, AND
        // 3. We haven't reset this period yet
        if current_month != period_month {
            // Handle edge case: reset_day > days in month
            let days_in_month = get_days_in_month(now.year(), current_month);
            let effective_reset_day = reset_day.min(days_in_month);
            
            if current_day >= effective_reset_day {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
}

fn get_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1|3|5|7|8|10|12 => 31,
        4|6|9|11 => 30,
        2 => if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) { 29 } else { 28 },
        _ => 30,
    }
}
```

### 4.2 Edge Cases

| Scenario | Solution |
|----------|----------|
| Reset day = 31, month has 30 days | Use last day of month (30) |
| Reset day = 29-31, February | Use Feb 28/29 |
| Reset at midnight UTC | Check at 00:00 UTC, reset on first request after |
| Multiple providers, different reset days | Each tracks independently |
| Timezone differences | Always use UTC internally |

---

## 5. Migration Strategy

### 5.1 Schema Versioning

```rust
// src/storage/migration.rs

const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn run_migrations(conn: &Connection) -> Result<(), String> {
    // Create schema_meta table if not exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    ).map_err(|e| e.to_string())?;
    
    // Get current version
    let version: u32 = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    
    // Run migrations
    let mut current = version;
    while current < CURRENT_SCHEMA_VERSION {
        current = run_migration(conn, current)?;
    }
    
    Ok(())
}

fn run_migration(conn: &Connection, from_version: u32) -> Result<u32, String> {
    match from_version {
        0 => {
            // Version 0: Initial schema
            conn.execute_batch(SCHEMA_V1).map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('version', '1')",
                [],
            ).map_err(|e| e.to_string())?;
            Ok(1)
        }
        // Future migrations:
        // 1 => { ... migrate to v2 ...; Ok(2) }
        _ => Err(format!("Unknown schema version: {}", from_version)),
    }
}
```

### 5.2 Future Migration Example

```rust
// Adding a new column in v2
const SCHEMA_V2_DIFF: &str = "
    ALTER TABLE request_logs ADD COLUMN reasoning_tokens INTEGER;
    ALTER TABLE request_logs ADD COLUMN reasoning_cost_usd REAL;
    
    CREATE INDEX IF NOT EXISTS idx_logs_reasoning ON request_logs(reasoning_tokens) 
        WHERE reasoning_tokens IS NOT NULL;
";

fn migrate_v1_to_v2(conn: &Connection) -> Result<(), String> {
    // SQLite doesn't support IF NOT EXISTS for ALTER TABLE
    // Check if column exists first
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('request_logs') WHERE name = 'reasoning_tokens'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    
    if !has_column {
        conn.execute_batch(SCHEMA_V2_DIFF).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
```

---

## 6. Error Handling & Recovery

### 6.1 Error Categories

```rust
// src/storage/error.rs

#[derive(Debug)]
pub enum StorageError {
    /// Database connection failed
    ConnectionError(String),
    /// Query execution failed
    QueryError(String),
    /// Transaction failed
    TransactionError(String),
    /// Lock acquisition timeout
    LockTimeout,
    /// Data integrity error
    IntegrityError(String),
    /// Schema migration failed
    MigrationError(String),
}

impl StorageError {
    pub fn is_retriable(&self) -> bool {
        matches!(self, StorageError::LockTimeout | StorageError::ConnectionError(_))
    }
}
```

### 6.2 Retry Logic

```rust
impl TokenStorage {
    fn with_retry<T, F>(&self, op: F) -> Result<T, StorageError>
    where
        F: Fn() -> Result<T, StorageError>,
    {
        let mut attempts = 0;
        let max_attempts = 3;
        let delay = std::time::Duration::from_millis(100);
        
        loop {
            match op() {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retriable() && attempts < max_attempts => {
                    attempts += 1;
                    tracing::warn!(
                        attempt = attempts,
                        error = ?e,
                        "Retrying storage operation"
                    );
                    std::thread::sleep(delay * attempts as u32);
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

### 6.3 Graceful Degradation

```rust
// In proxy.rs

// If storage fails, continue without token tracking
match state.storage.insert_log(&log_entry) {
    Ok(()) => {}
    Err(e) => {
        tracing::error!(
            error = %e,
            request_id = %request_id,
            "Failed to persist log entry - data will be missing from statistics"
        );
        // Continue - don't fail the request
    }
}

// Track in memory ring buffer as backup
state.log_buffer.push(log_entry.clone());
```

---

## 7. Cached Tokens & Prompt Caching

### 7.1 Provider Support

| Provider | Feature | Field Name |
|----------|---------|------------|
| Anthropic | Prompt Caching | `cache_read_tokens`, `cache_creation_tokens` |
| OpenAI | Not yet | - |
| DashScope | Not yet | - |
| Zhipu | Not yet | - |

### 7.2 Cost Calculation with Cache

```rust
impl PricingConfig {
    /// Calculate cost considering cached tokens
    pub fn calculate_cost_with_cache(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: Option<u32>,
    ) -> f64 {
        let (input_price, output_price) = self.find_pricing(model);
        
        // Standard cost
        let input_cost = (input_tokens as f64 / 1000.0) * input_price;
        let output_cost = (output_tokens as f64 / 1000.0) * output_price;
        
        // Deduct cached tokens (typically 90% discount)
        let cache_savings = cached_tokens
            .map(|c| (c as f64 / 1000.0) * input_price * 0.9)
            .unwrap_or(0.0);
        
        (input_cost + output_cost - cache_savings).max(0.0)
    }
}
```

### 7.3 Reporting Cached Savings

Add to API response:

```json
{
  "totalTokens": 100000,
  "cachedTokens": 20000,
  "cacheSavingsUsd": 0.27,
  "effectiveCostUsd": 1.23
}
```

---

## 8. UI Interaction Details

### 8.1 Form Validation Rules

**Quotas Page**:

| Field | Validation | Error Message |
|-------|------------|---------------|
| monthlyTokenBudget | > 0, < 10^12 | "Budget must be between 1 and 1 trillion tokens" |
| monthlyCostBudgetUsd | > 0, < 10^6 | "Budget must be between $0.01 and $1,000,000" |
| alertThreshold | 0.5 - 1.0 | "Threshold must be between 50% and 100%" |
| resetDay | 1 - 28 | "Reset day must be between 1 and 28 (to handle all months)" |

**Pricing Page**:

| Field | Validation | Error Message |
|-------|------------|---------------|
| modelPattern | Non-empty, valid glob | "Pattern cannot be empty" |
| inputPricePer1k | >= 0 | "Price cannot be negative" |
| outputPricePer1k | >= 0 | "Price cannot be negative" |

### 8.2 Loading States

```typescript
// UI state machine
type LoadingState = 
  | { type: 'idle' }
  | { type: 'loading' }
  | { type: 'success' }
  | { type: 'error', message: string };

// Component
const [state, setState] = useState<LoadingState>({ type: 'idle' });

// On fetch
setState({ type: 'loading' });
try {
  const data = await fetchTokens();
  setState({ type: 'success' });
} catch (e) {
  setState({ type: 'error', message: e.message });
}

// Render
{state.type === 'loading' && <Skeleton />}
{state.type === 'error' && <Alert variant="error">{state.message}</Alert>}
```

### 8.3 Error Display

```typescript
// Error boundary with retry
const TokenPageWithErrorBoundary = () => (
  <ErrorBoundary
    fallback={({ error, resetErrorBoundary }) => (
      <div className="error-container">
        <Alert variant="error">
          <AlertTitle>Failed to load token statistics</AlertTitle>
          <AlertDescription>{error.message}</AlertDescription>
        </Alert>
        <Button onClick={resetErrorBoundary}>Try Again</Button>
      </div>
    )}
  >
    <TokensPage />
  </ErrorBoundary>
);
```

### 8.4 Real-time Updates

```typescript
// WebSocket integration for real-time stats
const useTokenUpdates = () => {
  const { lastMessage } = useWebSocket('/zz/ws');
  const queryClient = useQueryClient();
  
  useEffect(() => {
    if (!lastMessage) return;
    
    const msg = JSON.parse(lastMessage.data);
    
    if (msg.type === 'token_update') {
      // Invalidate queries to refetch
      queryClient.invalidateQueries(['tokenSummary']);
      queryClient.invalidateQueries(['tokenTimeseries']);
    }
    
    if (msg.type === 'quota_alert') {
      // Show toast notification
      toast.warning(msg.data.message, {
        action: {
          label: 'View Quotas',
          onClick: () => navigate('/quotas'),
        },
      });
    }
  }, [lastMessage, queryClient]);
};
```

### 8.5 Export Implementation

```typescript
// CSV Export
const exportAsCsv = (data: TokenLog[], filename: string) => {
  const headers = ['timestamp', 'provider', 'model', 'input_tokens', 'output_tokens', 'cost_usd'];
  const rows = data.map(row => [
    row.timestamp,
    row.provider,
    row.model,
    row.inputTokens?.toString() ?? '',
    row.outputTokens?.toString() ?? '',
    row.costUsd?.toFixed(4) ?? '',
  ]);
  
  const csv = [
    headers.join(','),
    ...rows.map(r => r.map(escapeCsv).join(','))
  ].join('\n');
  
  downloadFile(csv, filename, 'text/csv');
};

const escapeCsv = (value: string): string => {
  if (value.includes(',') || value.includes('"') || value.includes('\n')) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
};

const downloadFile = (content: string, filename: string, mimeType: string) => {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};
```

---

## 9. Performance Benchmarks

### 9.1 Database Performance

| Operation | Target | Notes |
|-----------|--------|-------|
| Insert single log | < 5ms | With prepared statement |
| Insert batch (100) | < 50ms | Transaction-wrapped |
| Query timeseries (24h) | < 50ms | Indexed on timestamp |
| Query timeseries (90d) | < 200ms | Uses hourly aggregation |
| Query summary | < 100ms | Pre-computed aggregates |
| Update usage | < 5ms | Single row update |

### 9.2 Database Size Estimation

| Scenario | Size (90 days) |
|----------|----------------|
| 1,000 req/day | ~50 MB |
| 10,000 req/day | ~500 MB |
| 100,000 req/day | ~5 GB |

**Mitigation**: Use aggregation tables, delete raw logs after aggregation.

### 9.3 Query Optimization

```sql
-- Use EXPLAIN QUERY PLAN to verify index usage
EXPLAIN QUERY PLAN
SELECT * FROM request_logs 
WHERE timestamp BETWEEN '2026-03-01' AND '2026-03-22'
AND provider = 'ali-account-1';

-- Expected: USING INDEX idx_logs_provider_ts

-- If not using index, analyze:
ANALYZE;

-- For large queries, use covering index:
CREATE INDEX idx_logs_covering ON request_logs(
    timestamp, provider, model, input_tokens, output_tokens, cost_usd
);
```

---

## 10. Security Considerations

### 10.1 API Key Protection

- API keys in config are NOT stored in token database
- Only provider NAME is logged, never API key
- UI masks API keys by default

### 10.2 Database File Security

```rust
// Set restrictive permissions on database file
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn set_db_permissions(path: &str) -> Result<(), std::io::Error> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);  // Read/write for owner only
    fs::set_permissions(path, perms)
}
```

### 10.3 Input Validation

```rust
// Sanitize user inputs before SQL
// Using rusqlite's parameterized queries already prevents SQL injection
// But validate ranges and formats

fn validate_provider_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Provider name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("Provider name too long (max 64 chars)".to_string());
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err("Provider name can only contain alphanumeric, dash, underscore".to_string());
    }
    Ok(())
}
```

---

## 11. Monitoring & Observability

### 11.1 Metrics to Track

```rust
// Metrics exposed via /zz/api/metrics (Prometheus format)
pub fn collect_metrics(storage: &TokenStorage) -> String {
    format!(
        r#"
# HELP token_requests_total Total requests tracked
# TYPE token_requests_total counter
token_requests_total {}

# HELP token_input_bytes_total Total input tokens
# TYPE token_input_bytes_total counter
token_input_bytes_total {}

# HELP token_output_bytes_total Total output tokens
# TYPE token_output_bytes_total counter
token_output_bytes_total {}

# HELP token_cost_usd_total Total cost in USD
# TYPE token_cost_usd_total counter
token_cost_usd_total {:.4}

# HELP storage_operations_total Storage operations
# TYPE storage_operations_total counter
storage_operations_total{{operation="insert"}} {}
storage_operations_total{{operation="query"}} {}

# HELP storage_errors_total Storage errors
# TYPE storage_errors_total counter
storage_errors_total {}
"#,
        storage.get_total_requests(),
        storage.get_total_input_tokens(),
        storage.get_total_output_tokens(),
        storage.get_total_cost(),
        storage.metrics.inserts.load(Ordering::Relaxed),
        storage.metrics.queries.load(Ordering::Relaxed),
        storage.metrics.errors.load(Ordering::Relaxed),
    )
}
```

### 11.2 Health Check

```rust
// Enhanced health check includes storage status
pub async fn health_check(state: &AppState) -> HealthStatus {
    let storage_ok = state.storage.health_check().is_ok();
    
    HealthStatus {
        status: if storage_ok { "ok" } else { "degraded" },
        uptime_secs: state.start_time.elapsed().as_secs(),
        storage: if storage_ok { "healthy" } else { "error" },
        providers: state.provider_manager.get_all_states(),
    }
}
```

---

## 12. Appendix: Complete Error Code Reference

| Code | HTTP Status | Description | User Action |
|------|-------------|-------------|-------------|
| E001 | 400 | Invalid time range | Check start/end format (ISO 8601) |
| E002 | 400 | Invalid granularity | Use minute/hour/day/week/month |
| E003 | 400 | Provider not found | Check provider name |
| E004 | 400 | Invalid quota value | Check budget > 0, threshold 0.5-1.0 |
| E005 | 400 | Invalid pricing pattern | Check glob syntax |
| E006 | 500 | Storage unavailable | Check database file permissions |
| E007 | 500 | Migration failed | Check logs, may need manual intervention |
| E008 | 503 | Write queue full | Reduce request rate or increase batch size |

---

**Document Version**: 1.0
**Last Updated**: 2026-03-22
**Next Review**: After Phase 1 implementation