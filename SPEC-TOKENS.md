# ZZ - Token Consumption Statistics System Specification

## Version: 1.0.0
## Status: Draft
## Last Updated: 2026-03-22

---

## 1. Overview

### 1.1 Purpose

A comprehensive token consumption tracking and analysis system for the ZZ proxy. It serves two purposes:
1. **Internal Quota Management**: Real-time token counting for failover decisions and quota thresholds
2. **Independent Statistical Analysis**: Historical data analysis, cost calculation, and trend visualization

### 1.2 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Token extraction from API responses | Request body modification |
| SQLite persistence | Multi-node aggregation |
| Cost calculation with configurable pricing | Real-time billing integration |
| Time-series statistics (minute/hour/day/week/month) | Predictive analytics |
| Quota management with alerts | Third-party export (only CSV/JSON) |
| UI dashboards and charts | Mobile UI |

### 1.3 Dependencies

- Existing: `stats.rs`, `proxy.rs`, `admin_api.rs`, WebSocket infrastructure
- New: SQLite database, pricing configuration, storage layer

---

## 2. Data Model

### 2.1 Extended LogEntry

**File**: `src/stats.rs`

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    // Existing fields (DO NOT MODIFY)
    pub id: String,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub provider: String,
    pub status: u16,
    pub duration_ms: u64,
    pub ttfb_ms: u64,
    pub model: String,
    pub streaming: bool,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub failover_chain: Option<Vec<String>>,
    
    // NEW FIELDS (nullable for backward compatibility)
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cached_tokens: Option<u32>,
    pub cost_usd: Option<f64>,
}
```

**Acceptance Criteria**:
- [ ] New fields are optional (backward compatible with existing logs)
- [ ] Serialization to JSON includes new fields when present
- [ ] Deserialization handles missing fields gracefully (None)

### 2.2 SQLite Schema

**File**: `src/storage/schema.sql` (embedded in binary)

```sql
-- Request logs table
CREATE TABLE IF NOT EXISTS request_logs (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,              -- ISO 8601
    method TEXT,
    path TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    status INTEGER,
    duration_ms INTEGER,
    ttfb_ms INTEGER,
    streaming INTEGER,                    -- 0 or 1
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    cached_tokens INTEGER,
    cost_usd REAL,
    request_bytes INTEGER,
    response_bytes INTEGER,
    failover_chain TEXT                   -- JSON array or NULL
);

-- Indexes for query performance
CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_logs_provider ON request_logs(provider);
CREATE INDEX IF NOT EXISTS idx_logs_model ON request_logs(model);
CREATE INDEX IF NOT EXISTS idx_logs_provider_ts ON request_logs(provider, timestamp);
CREATE INDEX IF NOT EXISTS idx_logs_model_ts ON request_logs(model, timestamp);

-- Pre-aggregated hourly statistics
CREATE TABLE IF NOT EXISTS hourly_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hour_start TEXT NOT NULL,             -- ISO 8601, rounded to hour
    provider TEXT NOT NULL,
    model TEXT,                           -- NULL means "all models"
    request_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    total_input_tokens INTEGER DEFAULT 0,
    total_output_tokens INTEGER DEFAULT 0,
    total_tokens INTEGER DEFAULT 0,
    total_cached_tokens INTEGER DEFAULT 0,
    total_cost_usd REAL DEFAULT 0.0,
    avg_duration_ms REAL,
    avg_ttfb_ms REAL,
    total_request_bytes INTEGER DEFAULT 0,
    total_response_bytes INTEGER DEFAULT 0,
    UNIQUE(hour_start, provider, model)
);

CREATE INDEX IF NOT EXISTS idx_hourly_hour ON hourly_stats(hour_start);
CREATE INDEX IF NOT EXISTS idx_hourly_provider ON hourly_stats(provider);

-- Daily statistics (for faster monthly/yearly queries)
CREATE TABLE IF NOT EXISTS daily_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day_start TEXT NOT NULL,              -- ISO 8601 date (YYYY-MM-DD)
    provider TEXT NOT NULL,
    model TEXT,
    request_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    error_count INTEGER DEFAULT 0,
    total_input_tokens INTEGER DEFAULT 0,
    total_output_tokens INTEGER DEFAULT 0,
    total_tokens INTEGER DEFAULT 0,
    total_cached_tokens INTEGER DEFAULT 0,
    total_cost_usd REAL DEFAULT 0.0,
    avg_duration_ms REAL,
    UNIQUE(day_start, provider, model)
);

-- Provider quota configuration
CREATE TABLE IF NOT EXISTS provider_quotas (
    provider TEXT PRIMARY KEY,
    monthly_token_budget INTEGER,
    monthly_cost_budget_usd REAL,
    alert_threshold REAL DEFAULT 0.8,     -- Alert at 80% usage
    reset_day INTEGER DEFAULT 1,          -- Day of month to reset (1-28)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Current period usage (reset monthly)
CREATE TABLE IF NOT EXISTS current_usage (
    provider TEXT PRIMARY KEY,
    period_start TEXT NOT NULL,           -- Start of current billing period
    tokens_used INTEGER DEFAULT 0,
    cost_used_usd REAL DEFAULT 0.0,
    request_count INTEGER DEFAULT 0,
    updated_at TEXT NOT NULL
);

-- Model pricing configuration
CREATE TABLE IF NOT EXISTS model_pricing (
    model_pattern TEXT PRIMARY KEY,       -- Glob pattern or exact match
    input_price_per_1k REAL NOT NULL,     -- USD per 1000 input tokens
    output_price_per_1k REAL NOT NULL,    -- USD per 1000 output tokens
    effective_from TEXT NOT NULL,         -- ISO 8601
    effective_until TEXT,                 -- NULL means current
    created_at TEXT NOT NULL
);

-- Metadata table for schema versioning
CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_meta (key, value) VALUES ('version', '1');
```

**Acceptance Criteria**:
- [ ] Database created at configured path on first startup
- [ ] Schema migration handles version upgrades
- [ ] All indexes created successfully
- [ ] Database file has appropriate permissions (0600)

### 2.3 TypeScript Types

**File**: `ui/src/api/types.ts`

```typescript
// Token statistics
export interface TokenStats {
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  totalCostUsd: number;
  requestCount: number;
  successCount: number;
  errorCount: number;
  avgDurationMs: number;
}

export interface TimeSeriesPoint {
  time: string;              // ISO 8601
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  costUsd: number;
  requestCount: number;
}

export interface ProviderTokenStats extends TokenStats {
  provider: string;
  quota?: QuotaInfo;
}

export interface ModelTokenStats extends TokenStats {
  model: string;
  avgInputTokens: number;
  avgOutputTokens: number;
}

export interface QuotaInfo {
  provider: string;
  monthlyTokenBudget: number | null;
  monthlyCostBudgetUsd: number | null;
  tokensUsed: number;
  costUsedUsd: number;
  usagePercent: number;
  alertThreshold: number;
  resetDay: number;
  periodStart: string;
  daysUntilReset: number;
}

export interface ModelPricing {
  modelPattern: string;
  inputPricePer1k: number;
  outputPricePer1k: number;
  effectiveFrom: string;
  effectiveUntil: string | null;
}

export interface StatsFilter {
  startTime?: string;        // ISO 8601
  endTime?: string;          // ISO 8601
  provider?: string;
  model?: string;
  granularity?: 'minute' | 'hour' | 'day' | 'week' | 'month';
}

export interface TokenSummary {
  today: TokenStats;
  yesterday: TokenStats;
  thisWeek: TokenStats;
  thisMonth: TokenStats;
  lastMonth: TokenStats;
}
```

---

## 3. Backend Implementation

### 3.1 Token Extraction

**File**: `src/token_extractor.rs` (NEW)

```rust
use serde_json::Value;

/// Token usage extracted from API response
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub cached_tokens: Option<u32>,
}

/// Extract token usage from OpenAI-compatible response body
pub fn extract_usage(body: &[u8]) -> Option<TokenUsage> {
    // Parse JSON
    let v: Value = serde_json::from_slice(body).ok()?;
    
    // Navigate to usage field
    let usage = v.get("usage")?;
    
    let input = usage.get("prompt_tokens")?.as_u64()? as u32;
    let output = usage.get("completion_tokens")?.as_u64()? as u32;
    let total = usage.get("total_tokens")
        .map(|t| t.as_u64().unwrap_or(0) as u32)
        .unwrap_or(input + output);
    
    // Optional: cached tokens (some providers support this)
    let cached = usage.get("cached_tokens")
        .and_then(|c| c.as_u64())
        .map(|c| c as u32);
    
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        cached_tokens: cached,
    })
}

/// Extract usage from streaming SSE chunks (final chunk contains usage)
pub fn extract_usage_from_sse(chunks: &[String]) -> Option<TokenUsage> {
    // SSE format: "data: {...}\n\n"
    // Look for chunk with "usage" field
    for chunk in chunks.iter().rev() {
        if let Some(json_str) = chunk.strip_prefix("data: ") {
            if json_str == "[DONE]" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(json_str) {
                // For streaming, usage is often in the final chunk
                // under "usage" or nested in choices
                if let Some(usage) = v.get("usage") {
                    return Some(TokenUsage {
                        input_tokens: usage.get("prompt_tokens")?.as_u64()? as u32,
                        output_tokens: usage.get("completion_tokens")?.as_u64()? as u32,
                        total_tokens: usage.get("total_tokens")
                            .map(|t| t.as_u64().unwrap_or(0) as u32)
                            .unwrap_or_default(),
                        cached_tokens: None,
                    });
                }
            }
        }
    }
    None
}
```

**Acceptance Criteria**:
- [ ] Successfully extracts usage from OpenAI-format JSON responses
- [ ] Handles missing fields gracefully (returns None)
- [ ] Works with DashScope, Zhipu GLM, OpenAI response formats
- [ ] Unit tests cover edge cases (malformed JSON, missing usage, etc.)

### 3.2 Storage Layer

**File**: `src/storage/mod.rs` (NEW)

```rust
pub mod schema;

use rusqlite::{Connection, params, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use crate::stats::LogEntry;
use crate::token_extractor::TokenUsage;

pub struct TokenStorage {
    conn: Mutex<Connection>,
    db_path: String,
}

impl TokenStorage {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", &"WAL")
            .map_err(|e| format!("Failed to set WAL mode: {}", e))?;
        
        // Run schema
        conn.execute_batch(schema::SCHEMA)
            .map_err(|e| format!("Failed to initialize schema: {}", e))?;
        
        Ok(Self {
            conn: Mutex::new(conn),
            db_path: db_path.to_string(),
        })
    }
    
    /// Insert a log entry with token data
    pub fn insert_log(&self, entry: &LogEntry) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        
        conn.execute(
            "INSERT OR REPLACE INTO request_logs (
                id, timestamp, method, path, provider, model, status,
                duration_ms, ttfb_ms, streaming, input_tokens, output_tokens,
                total_tokens, cached_tokens, cost_usd, request_bytes,
                response_bytes, failover_chain
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                entry.id,
                entry.timestamp,
                entry.method,
                entry.path,
                entry.provider,
                entry.model,
                entry.status,
                entry.duration_ms,
                entry.ttfb_ms,
                entry.streaming as i32,
                entry.input_tokens,
                entry.output_tokens,
                entry.total_tokens,
                entry.cached_tokens,
                entry.cost_usd,
                entry.request_bytes,
                entry.response_bytes,
                entry.failover_chain.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default()),
            ],
        ).map_err(|e| format!("Failed to insert log: {}", e))?;
        
        Ok(())
    }
    
    /// Query time series data
    pub fn query_timeseries(
        &self,
        start: &str,
        end: &str,
        granularity: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<Vec<TimeSeriesRow>, String> {
        // Implementation details...
    }
    
    /// Aggregate hourly stats (called by background task)
    pub fn aggregate_hourly(&self, hour: &str) -> Result<(), String> {
        // Implementation details...
    }
    
    /// Get current usage for quota tracking
    pub fn get_current_usage(&self, provider: &str) -> Result<CurrentUsage, String> {
        // Implementation details...
    }
    
    /// Update current usage
    pub fn update_usage(&self, provider: &str, tokens: u32, cost: f64) -> Result<(), String> {
        // Implementation details...
    }
}
```

**Acceptance Criteria**:
- [ ] Database operations are thread-safe (Mutex-protected)
- [ ] WAL mode enabled for concurrent read/write
- [ ] Query performance: time-series query < 100ms for 90-day range
- [ ] Insert performance: < 5ms per log entry
- [ ] Graceful handling of database errors

### 3.3 Cost Calculator

**File**: `src/pricing.rs` (NEW)

```rust
use std::collections::HashMap;

/// Model pricing configuration
pub struct PricingConfig {
    /// Model pattern -> (input_price_per_1k, output_price_per_1k)
    prices: HashMap<String, (f64, f64)>,
    /// Default pricing when no pattern matches
    default_input: f64,
    default_output: f64,
}

impl PricingConfig {
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut prices = HashMap::new();
        
        for (pattern, pricing) in &config.stats.pricing {
            prices.insert(
                pattern.clone(),
                (pricing.input_per_1k, pricing.output_per_1k),
            );
        }
        
        Self {
            prices,
            default_input: config.stats.default_input_per_1k,
            default_output: config.stats.default_output_per_1k,
        }
    }
    
    /// Calculate cost for a request
    pub fn calculate_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        let (input_price, output_price) = self.find_pricing(model);
        
        let input_cost = (input_tokens as f64 / 1000.0) * input_price;
        let output_cost = (output_tokens as f64 / 1000.0) * output_price;
        
        input_cost + output_cost
    }
    
    fn find_pricing(&self, model: &str) -> (f64, f64) {
        // Exact match first
        if let Some(&pricing) = self.prices.get(model) {
            return pricing;
        }
        
        // Glob pattern match (simple prefix/suffix matching)
        for (pattern, &pricing) in &self.prices {
            if Self::matches_pattern(pattern, model) {
                return pricing;
            }
        }
        
        (self.default_input, self.default_output)
    }
    
    fn matches_pattern(pattern: &str, model: &str) -> bool {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len()-1];
            model.starts_with(prefix)
        } else if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            model.ends_with(suffix)
        } else {
            pattern == model
        }
    }
}
```

**Acceptance Criteria**:
- [ ] Supports exact match and glob patterns (prefix*, *suffix, *contains*)
- [ ] Returns 0.0 cost when pricing not configured
- [ ] Loads pricing from config.toml on startup
- [ ] Hot-reload pricing when config changes

### 3.4 API Endpoints

**File**: `src/admin_api.rs` (EXTEND)

#### 3.4.1 Token Summary

```
GET /zz/api/tokens/summary
```

**Response** `200 OK`:
```json
{
  "today": {
    "totalTokens": 125000,
    "inputTokens": 85000,
    "outputTokens": 40000,
    "cachedTokens": 5000,
    "totalCostUsd": 12.50,
    "requestCount": 150,
    "successCount": 145,
    "errorCount": 5,
    "avgDurationMs": 2350
  },
  "yesterday": { ... },
  "thisWeek": { ... },
  "thisMonth": { ... },
  "lastMonth": { ... }
}
```

**Acceptance Criteria**:
- [ ] All time ranges return correct aggregations
- [ ] Performance: < 50ms response time
- [ ] Handles empty data gracefully (zeros)

#### 3.4.2 Time Series

```
GET /zz/api/tokens/timeseries?start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z&granularity=hour&provider=ali-account-1
```

**Query Parameters**:
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| start | ISO 8601 | Yes | Start timestamp |
| end | ISO 8601 | Yes | End timestamp |
| granularity | string | No | minute/hour/day/week/month (default: hour) |
| provider | string | No | Filter by provider |
| model | string | No | Filter by model |

**Response** `200 OK`:
```json
{
  "start": "2026-03-01T00:00:00Z",
  "end": "2026-03-22T00:00:00Z",
  "granularity": "hour",
  "provider": "ali-account-1",
  "data": [
    {
      "time": "2026-03-01T00:00:00Z",
      "inputTokens": 5000,
      "outputTokens": 2000,
      "totalTokens": 7000,
      "costUsd": 0.70,
      "requestCount": 10
    },
    // ...
  ]
}
```

**Acceptance Criteria**:
- [ ] Supports all granularity levels
- [ ] Returns empty array for no data in range
- [ ] Respects timezone (UTC)
- [ ] Max 1000 data points per response

#### 3.4.3 By Provider

```
GET /zz/api/tokens/by-provider?period=month
```

**Query Parameters**:
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| period | string | No | today/week/month (default: month) |
| includeQuota | boolean | No | Include quota info (default: true) |

**Response** `200 OK`:
```json
{
  "period": "month",
  "providers": [
    {
      "provider": "ali-account-1",
      "totalTokens": 500000,
      "inputTokens": 350000,
      "outputTokens": 150000,
      "cachedTokens": 10000,
      "totalCostUsd": 50.00,
      "requestCount": 600,
      "successCount": 590,
      "errorCount": 10,
      "avgDurationMs": 2100,
      "quota": {
        "provider": "ali-account-1",
        "monthlyTokenBudget": 1000000,
        "monthlyCostBudgetUsd": 100.00,
        "tokensUsed": 500000,
        "costUsedUsd": 50.00,
        "usagePercent": 50.0,
        "alertThreshold": 0.8,
        "resetDay": 1,
        "periodStart": "2026-03-01T00:00:00Z",
        "daysUntilReset": 10
      }
    }
  ]
}
```

**Acceptance Criteria**:
- [ ] Sorted by token usage (descending)
- [ ] Quota info included when configured
- [ ] Days until reset calculated correctly

#### 3.4.4 By Model

```
GET /zz/api/tokens/by-model?period=month
```

**Response** `200 OK`:
```json
{
  "period": "month",
  "models": [
    {
      "model": "qwen-plus",
      "totalTokens": 300000,
      "inputTokens": 200000,
      "outputTokens": 100000,
      "cachedTokens": 5000,
      "totalCostUsd": 30.00,
      "requestCount": 400,
      "successCount": 395,
      "errorCount": 5,
      "avgDurationMs": 2000,
      "avgInputTokens": 500,
      "avgOutputTokens": 250
    }
  ]
}
```

**Acceptance Criteria**:
- [ ] Sorted by token usage (descending)
- [ ] Groups by exact model name
- [ ] Average tokens per request calculated

#### 3.4.5 Quota Management

```
GET /zz/api/quotas
PUT /zz/api/quotas
```

**PUT Request**:
```json
{
  "quotas": [
    {
      "provider": "ali-account-1",
      "monthlyTokenBudget": 1000000,
      "monthlyCostBudgetUsd": 100.00,
      "alertThreshold": 0.8,
      "resetDay": 1
    }
  ]
}
```

**Acceptance Criteria**:
- [ ] Quota changes take effect immediately
- [ ] Persists to database (not just config)
- [ ] Validation: resetDay must be 1-28

#### 3.4.6 Model Pricing

```
GET /zz/api/pricing
PUT /zz/api/pricing
```

**PUT Request**:
```json
{
  "pricing": [
    {
      "modelPattern": "claude-3-opus",
      "inputPricePer1k": 0.015,
      "outputPricePer1k": 0.075
    },
    {
      "modelPattern": "gpt-4*",
      "inputPricePer1k": 0.03,
      "outputPricePer1k": 0.06
    }
  ]
}
```

**Acceptance Criteria**:
- [ ] Pattern matching supports wildcards
- [ ] Changes take effect immediately for new requests
- [ ] Persists to database

#### 3.4.7 Export

```
GET /zz/api/tokens/export?format=csv&start=...&end=...
GET /zz/api/tokens/export?format=json&start=...&end=...
```

**Acceptance Criteria**:
- [ ] CSV has headers: timestamp, provider, model, input_tokens, output_tokens, cost_usd
- [ ] JSON is array of objects with all fields
- [ ] Max export: 100,000 records
- [ ] Streaming response for large exports

### 3.5 WebSocket Extensions

**File**: `src/ws.rs` (EXTEND)

#### 3.5.1 Token Event

```json
{
  "type": "token_update",
  "data": {
    "provider": "ali-account-1",
    "inputTokens": 500,
    "outputTokens": 200,
    "costUsd": 0.05,
    "timestamp": "2026-03-22T10:30:00Z"
  }
}
```

#### 3.5.2 Quota Alert Event

```json
{
  "type": "quota_alert",
  "data": {
    "provider": "ali-account-1",
    "usagePercent": 85.5,
    "threshold": 80.0,
    "tokensUsed": 855000,
    "tokenBudget": 1000000,
    "message": "Provider ali-account-1 has exceeded 80% quota usage"
  }
}
```

**Acceptance Criteria**:
- [ ] Token update sent after each successful request with token data
- [ ] Quota alert sent when usage exceeds threshold
- [ ] No duplicate alerts within same period

### 3.6 Background Tasks

**File**: `src/tasks.rs` (NEW)

```rust
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Aggregate hourly statistics
pub async fn aggregation_task(storage: Arc<TokenStorage>) {
    let mut interval = interval(Duration::from_secs(3600)); // Every hour
    
    loop {
        interval.tick().await;
        
        let previous_hour = chrono::Utc::now() - chrono::Duration::hours(1);
        let hour_str = previous_hour.format("%Y-%m-%dT%H:00:00Z").to_string();
        
        if let Err(e) = storage.aggregate_hourly(&hour_str) {
            tracing::error!("Failed to aggregate hourly stats: {}", e);
        }
    }
}

/// Daily aggregation (for fast monthly queries)
pub async fn daily_aggregation_task(storage: Arc<TokenStorage>) {
    let mut interval = interval(Duration::from_secs(86400)); // Daily
    
    loop {
        interval.tick().await;
        
        let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
        let day_str = yesterday.format("%Y-%m-%d").to_string();
        
        if let Err(e) = storage.aggregate_daily(&day_str) {
            tracing::error!("Failed to aggregate daily stats: {}", e);
        }
    }
}

/// Periodic usage reset (on configured reset day)
pub async fn quota_reset_task(storage: Arc<TokenStorage>) {
    // Check every hour if it's reset day
    let mut interval = interval(Duration::from_secs(3600));
    
    loop {
        interval.tick().await;
        
        let now = chrono::Utc::now();
        // Check each provider's reset day and reset if needed
        if let Err(e) = storage.check_and_reset_quotas(now) {
            tracing::error!("Failed to reset quotas: {}", e);
        }
    }
}
```

**Acceptance Criteria**:
- [ ] Hourly aggregation runs at minute 0 of each hour
- [ ] Daily aggregation runs at 00:05 UTC
- [ ] Quota reset runs at first request after midnight on reset day
- [ ] All tasks handle errors gracefully without crashing

### 3.7 Quota-Aware Routing Integration

**File**: `src/router.rs` (EXTEND)

```rust
impl Router {
    /// Select provider considering quota limits
    pub fn select_provider_quota_aware(
        &self,
        providers: &[(String, Arc<Provider>)],
        storage: &TokenStorage,
    ) -> Option<(String, Arc<Provider>)> {
        for (name, provider) in providers {
            // Check if provider has quota configured
            if let Ok(Some(quota)) = storage.get_quota(name) {
                // Check if quota exceeded
                if let Ok(usage) = storage.get_current_usage(name) {
                    if let Some(budget) = quota.monthly_token_budget {
                        if usage.tokens_used >= budget {
                            tracing::warn!(
                                provider = %name,
                                used = usage.tokens_used,
                                budget = budget,
                                "Provider quota exhausted, skipping"
                            );
                            continue; // Skip this provider
                        }
                        
                        // Check if approaching limit
                        let usage_pct = usage.tokens_used as f64 / budget as f64;
                        if usage_pct >= quota.alert_threshold {
                            tracing::warn!(
                                provider = %name,
                                usage_pct = format!("{:.1}%", usage_pct * 100.0),
                                "Provider approaching quota limit"
                            );
                        }
                    }
                }
            }
            
            // Provider has quota room or no quota configured
            return Some((name.clone(), provider.clone()));
        }
        
        None
    }
}
```

**Acceptance Criteria**:
- [ ] Quota-aware strategy respects token budgets
- [ ] Providers at quota limit are skipped
- [ ] Warning logged when approaching threshold
- [ ] Works with existing failover logic

---

## 4. Configuration

### 4.1 Config Extension

**File**: `config.toml`

```toml
[server]
# ... existing ...

# Token Statistics Configuration
[stats]
enabled = true                              # Enable token tracking
db_path = "./zz_stats.db"                  # SQLite database path
retention_days = 90                         # Delete logs older than N days (0 = forever)
enable_cost_calculation = true              # Calculate costs
default_input_per_1k = 0.001               # Default $/1k input tokens
default_output_per_1k = 0.002              # Default $/1k output tokens

# Model pricing (optional, overrides defaults)
[stats.pricing.claude-3-opus]
input_per_1k = 0.015
output_per_1k = 0.075

[stats.pricing.claude-3-sonnet]
input_per_1k = 0.003
output_per_1k = 0.015

[stats.pricing.gpt-4]
input_per_1k = 0.03
output_per_1k = 0.06

[stats.pricing.gpt-4-turbo]
input_per_1k = 0.01
output_per_1k = 0.03

[stats.pricing.qwen-plus]
input_per_1k = 0.0004
output_per_1k = 0.0012

[stats.pricing.glm-4]
input_per_1k = 0.0014
output_per_1k = 0.0014

# Provider quotas (can also be managed via API)
[[quotas]]
provider = "ali-account-1"
monthly_token_budget = 1000000            # 1M tokens/month
monthly_cost_budget_usd = 50.00           # $50/month
alert_threshold = 0.8                      # Alert at 80%
reset_day = 1                              # Reset on 1st of each month

[[quotas]]
provider = "zhipu-account-1"
monthly_token_budget = 500000
monthly_cost_budget_usd = 30.00
alert_threshold = 0.9
reset_day = 1
```

### 4.2 Config Struct Extension

**File**: `src/config.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct StatsConfig {
    pub enabled: bool,
    pub db_path: String,
    pub retention_days: u32,
    pub enable_cost_calculation: bool,
    pub default_input_per_1k: f64,
    pub default_output_per_1k: f64,
    pub pricing: std::collections::HashMap<String, PricingEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingEntry {
    pub input_per_1k: f64,
    pub output_per_1k: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuotaConfig {
    pub provider: String,
    pub monthly_token_budget: Option<u64>,
    pub monthly_cost_budget_usd: Option<f64>,
    pub alert_threshold: f64,
    pub reset_day: u8,
}

// Add to Config struct
pub struct Config {
    // ... existing ...
    pub stats: StatsConfig,
    pub quotas: Vec<QuotaConfig>,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: "./zz_stats.db".to_string(),
            retention_days: 90,
            enable_cost_calculation: true,
            default_input_per_1k: 0.001,
            default_output_per_1k: 0.002,
            pricing: std::collections::HashMap::new(),
        }
    }
}
```

---

## 5. UI Implementation

### 5.1 Page Structure

New pages added to existing UI:

```
ui/src/pages/
├── Overview.tsx      # EXISTING - Add token summary cards
├── Providers.tsx     # EXISTING
├── Routing.tsx       # EXISTING
├── Logs.tsx          # EXISTING - Add token columns
├── Config.tsx        # EXISTING
├── Tokens.tsx        # NEW - Main token analytics page
├── Quotas.tsx        # NEW - Quota management page
└── Pricing.tsx       # NEW - Model pricing config page
```

### 5.2 Tokens Page

**File**: `ui/src/pages/Tokens.tsx`

```
┌─────────────────────────────────────────────────────────────────┐
│  Token Analytics                                                │
├─────────────────────────────────────────────────────────────────┤
│  [Today] [Week] [Month] [Custom Range]                          │
├──────────┬──────────┬──────────┬──────────┬─────────────────────┤
│ Total    │ Input    │ Output   │ Cost     │ Requests            │
│ 1.25M    │ 850K     │ 400K     │ $12.50   │ 150                 │
│ ▲ 15%    │ ▲ 10%    │ ▲ 25%    │ ▲ 18%    │ ▲ 12%              │
├──────────┴──────────┴──────────┴──────────┴─────────────────────┤
│                                                                 │
│  Token Consumption Trend                                        │
│  ┌────────────────────────────────────────────────────────────┐│
│  │ ▲ Tokens                                                    ││
│  │ │    ╭──╮                                                   ││
│  │ │   ╭╯  ╰╮    ╭─╮                                         ││
│  │ │  ╭╯    ╰───╯  ╰╮                                        ││
│  │ │ ╭╯              ╰╮                                       ││
│  │ ├─┴─────────────────┴─────────────────────────────────────▶││
│  │   Mar 1  Mar 5  Mar 10  Mar 15  Mar 20  Mar 22             ││
│  └────────────────────────────────────────────────────────────┘│
│  Legend: ━━ Input  ── Output  ··· Cost                         │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  By Provider                    │ By Model                       │
│  ┌────────────────────────────┐ │ ┌────────────────────────────┐│
│  │ ali-account-1  45% ████████│ │ │ qwen-plus     60% ████████ ││
│  │ zhipu-1       30% █████    │ │ │ gpt-4         25% ████      ││
│  │ ali-account-2 25% ████     │ │ │ claude-3      15% ███       ││
│  └────────────────────────────┘ │ └────────────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│  Detailed Data                                      [Export ▾]  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Time       │ Provider  │ Model      │ Input  │ Output │Cost ││
│  ├────────────┼───────────┼────────────┼────────┼────────┼─────┤│
│  │ 03-22 14:00│ ali-1     │ qwen-plus  │ 5,000  │ 2,000  │$0.05││
│  │ 03-22 14:00│ zhipu-1   │ glm-4      │ 3,000  │ 1,500  │$0.01││
│  │ ...        │           │            │        │        │     ││
│  └────────────┴───────────┴────────────┴────────┴────────┴─────┘│
│                                              [1] 2 3 ... 10 >   │
└─────────────────────────────────────────────────────────────────┘
```

**Component Tree**:
```
TokensPage
├── TimeRangeSelector (tabs: today/week/month/custom)
├── StatsSummary (5 cards with trend indicators)
├── TokenTrendChart (Recharts LineChart)
├── DistributionCharts
│   ├── ProviderPieChart
│   └── ModelPieChart
├── DataTable
│   ├── TableHeader
│   ├── TableBody
│   └── Pagination
└── ExportButton (dropdown: CSV/JSON)
```

**Acceptance Criteria**:
- [ ] Time range selector updates all components
- [ ] Charts render correctly with real data
- [ ] Table pagination works (50 rows per page)
- [ ] Export downloads file with correct data
- [ ] Loading states during data fetch
- [ ] Error handling for failed API calls

### 5.3 Quotas Page

**File**: `ui/src/pages/Quotas.tsx`

```
┌─────────────────────────────────────────────────────────────────┐
│  Quota Management                                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ali-account-1                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Token Budget                                                 ││
│  │ [1,000,000] tokens/month                                     ││
│  │                                                              ││
│  │ Used: 750,000 (75%)        Remaining: 250,000                ││
│  │ [██████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░]    ││
│  │                                                              ││
│  │ Cost Budget                                                  ││
│  │ [$100.00] / month                                            ││
│  │                                                              ││
│  │ Used: $75.50 (75.5%)       Remaining: $24.50                 ││
│  │ [██████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░]    ││
│  │                                                              ││
│  │ Alert Threshold: [80]%     Reset Day: [1st] of each month   ││
│  │                                                              ││
│  │ Days until reset: 10                                         ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  zhipu-account-1                                                │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Token Budget: [500,000] tokens/month                         ││
│  │ Used: 150,000 (30%)        Remaining: 350,000                ││
│  │ [████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░]  ││
│  │ ...                                                          ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│                                         [Save All Changes]      │
└─────────────────────────────────────────────────────────────────┘
```

**Component Tree**:
```
QuotasPage
├── QuotaCard[] (one per provider)
│   ├── TokenBudgetSection
│   │   ├── BudgetInput
│   │   ├── UsageProgressBar
│   │   └── RemainingLabel
│   ├── CostBudgetSection
│   │   ├── BudgetInput
│   │   ├── UsageProgressBar
│   │   └── RemainingLabel
│   ├── AlertThresholdInput (slider 50-100%)
│   ├── ResetDaySelector (dropdown 1-28)
│   └── DaysUntilReset
└── SaveButton
```

**Acceptance Criteria**:
- [ ] Real-time progress bar updates
- [ ] Input validation (budgets > 0, threshold 50-100, reset day 1-28)
- [ ] Toast notification on save success
- [ ] Error handling for invalid inputs
- [ ] Visual warning when usage > threshold (amber/red)

### 5.4 Pricing Page

**File**: `ui/src/pages/Pricing.tsx`

```
┌─────────────────────────────────────────────────────────────────┐
│  Model Pricing                                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Default Pricing (fallback when no pattern matches)             │
│  Input: [$0.001] per 1k tokens  Output: [$0.002] per 1k tokens │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  Model-Specific Pricing                          [+ Add Pricing]│
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Model Pattern    │ Input ($/1k) │ Output ($/1k) │ Actions  ││
│  ├──────────────────┼──────────────┼───────────────┼──────────┤│
│  │ claude-3-opus    │ $0.015       │ $0.075        │ [Edit][x]││
│  │ claude-3-sonnet  │ $0.003       │ $0.015        │ [Edit][x]││
│  │ gpt-4*           │ $0.030       │ $0.060        │ [Edit][x]││
│  │ qwen-*           │ $0.0004      │ $0.0012       │ [Edit][x]││
│  └──────────────────┴──────────────┴───────────────┴──────────┘│
│                                                                 │
│  Pattern Guide:                                                 │
│  • Exact match: "gpt-4" matches only "gpt-4"                    │
│  • Prefix wildcard: "gpt-4*" matches "gpt-4", "gpt-4-turbo"     │
│  • Suffix wildcard: "*-turbo" matches "gpt-4-turbo"             │
│                                                                 │
│                                         [Save All Changes]      │
└─────────────────────────────────────────────────────────────────┘
```

**Component Tree**:
```
PricingPage
├── DefaultPricingSection
│   ├── InputPriceInput
│   └── OutputPriceInput
├── PricingTable
│   ├── TableHeader
│   └── PricingRow[]
│       ├── ModelPatternInput
│       ├── InputPriceInput
│       ├── OutputPriceInput
│       ├── EditButton
│       └── DeleteButton
├── AddPricingButton
├── PatternGuide (help text)
└── SaveButton
```

**Acceptance Criteria**:
- [ ] Pattern syntax validation
- [ ] Preview of matching models (optional)
- [ ] Inline editing with validation
- [ ] Delete confirmation
- [ ] Toast on save

### 5.5 Overview Page Updates

**File**: `ui/src/pages/Overview.tsx` (EXTEND)

Add token summary section below existing stats:

```
├─────────────────────────────────────────────────────────────────┤
│  Token Summary (Today)                                          │
├──────────┬──────────┬──────────┬────────────────────────────────┤
│ Tokens   │ Cost     │ Top Provider │ Top Model                  │
│ 125K     │ $1.25    │ ali-1 (45%)  │ qwen-plus (60%)            │
└──────────┴──────────┴──────────┴────────────────────────────────┘
```

**Acceptance Criteria**:
- [ ] Shows today's token count and cost
- [ ] Links to Tokens page on click
- [ ] Updates via WebSocket

### 5.6 Logs Page Updates

**File**: `ui/src/pages/Logs.tsx` (EXTEND)

Add token columns to log table:

```
│ Time       │ Provider │ Model     │ Input │ Output │ Cost  │ St │
│ 14:05:02   │ ali-1    │ qwen-plus │ 500   │ 200    │ $0.01 │ 200│
```

**Acceptance Criteria**:
- [ ] Token columns hidden by default, toggleable
- [ ] "N/A" shown when token data not available
- [ ] Cost formatted as USD

### 5.7 Navigation Updates

**File**: `ui/src/components/layout/Layout.tsx` (EXTEND)

Add new navigation items:

```typescript
const navItems = [
  { path: '/', label: 'Overview' },
  { path: '/providers', label: 'Providers' },
  { path: '/tokens', label: 'Tokens' },      // NEW
  { path: '/quotas', label: 'Quotas' },      // NEW
  { path: '/routing', label: 'Routing' },
  { path: '/logs', label: 'Logs' },
  { path: '/config', label: 'Config' },
];
```

---

## 6. Testing

### 6.1 Unit Tests

**File**: `src/token_extractor.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_usage_openai_format() {
        let body = br#"{
            "id": "chatcmpl-123",
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        }"#;
        
        let usage = extract_usage(body).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }
    
    #[test]
    fn test_extract_usage_missing_usage() {
        let body = br#"{"id": "chatcmpl-123"}"#;
        assert!(extract_usage(body).is_none());
    }
    
    #[test]
    fn test_extract_usage_malformed_json() {
        let body = b"not json";
        assert!(extract_usage(body).is_none());
    }
    
    #[test]
    fn test_extract_usage_with_cached_tokens() {
        let body = br#"{
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150,
                "cached_tokens": 20
            }
        }"#;
        
        let usage = extract_usage(body).unwrap();
        assert_eq!(usage.cached_tokens, Some(20));
    }
}
```

**File**: `src/pricing.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exact_match() {
        let mut config = PricingConfig::default();
        config.prices.insert("gpt-4".to_string(), (0.03, 0.06));
        
        let cost = config.calculate_cost("gpt-4", 1000, 500);
        // Input: 1k * 0.03 = 0.03, Output: 0.5k * 0.06 = 0.03
        assert!((cost - 0.06).abs() < 0.0001);
    }
    
    #[test]
    fn test_prefix_wildcard() {
        let mut config = PricingConfig::default();
        config.prices.insert("gpt-4*".to_string(), (0.03, 0.06));
        
        let cost = config.calculate_cost("gpt-4-turbo", 1000, 500);
        assert!((cost - 0.06).abs() < 0.0001);
    }
    
    #[test]
    fn test_default_pricing() {
        let config = PricingConfig::default();
        
        let cost = config.calculate_cost("unknown-model", 1000, 1000);
        // Default: 0.001 + 0.002 = 0.003
        assert!((cost - 0.003).abs() < 0.0001);
    }
}
```

### 6.2 Integration Tests

**File**: `tests/token_api_tests.rs`

```rust
#[tokio::test]
async fn test_token_summary_endpoint() {
    let app = spawn_app().await;
    
    let response = app.get("/zz/api/tokens/summary").await;
    assert_eq!(response.status(), 200);
    
    let body: TokenSummary = response.json().await;
    assert!(body.today.total_tokens >= 0);
}

#[tokio::test]
async fn test_timeseries_query() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/timeseries")
        .query("start", "2026-03-01T00:00:00Z")
        .query("end", "2026-03-22T00:00:00Z")
        .query("granularity", "hour")
        .await;
    
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_quota_crud() {
    let app = spawn_app().await;
    
    // Create quota
    let quota = json!({
        "provider": "test-provider",
        "monthly_token_budget": 1000000,
        "alert_threshold": 0.8,
        "reset_day": 1
    });
    
    let response = app.put("/zz/api/quotas").json(&quota).await;
    assert_eq!(response.status(), 200);
    
    // Read quota
    let response = app.get("/zz/api/quotas").await;
    let body = response.json::<QuotaList>().await;
    assert!(body.quotas.iter().any(|q| q.provider == "test-provider"));
}
```

### 6.3 E2E Tests (UI)

**File**: `ui/src/__tests__/Tokens.test.tsx`

```typescript
import { render, screen, waitFor } from '@testing-library/react';
import { TokensPage } from '../pages/Tokens';

describe('TokensPage', () => {
  it('renders token summary cards', async () => {
    render(<TokensPage />);
    
    await waitFor(() => {
      expect(screen.getByText('Total')).toBeInTheDocument();
      expect(screen.getByText('Input')).toBeInTheDocument();
      expect(screen.getByText('Output')).toBeInTheDocument();
    });
  });
  
  it('handles time range selection', async () => {
    render(<TokensPage />);
    
    const weekTab = screen.getByRole('tab', { name: /week/i });
    fireEvent.click(weekTab);
    
    await waitFor(() => {
      // Verify API called with correct params
    });
  });
  
  it('exports data as CSV', async () => {
    render(<TokensPage />);
    
    const exportBtn = screen.getByRole('button', { name: /export/i });
    fireEvent.click(exportBtn);
    
    const csvOption = screen.getByText('CSV');
    fireEvent.click(csvOption);
    
    // Verify download triggered
  });
});
```

---

## 7. Acceptance Criteria Summary

### 7.1 Backend

| ID | Criteria | Priority |
|----|----------|----------|
| BE-01 | Token extraction works for OpenAI/DashScope/Zhipu responses | P0 |
| BE-02 | SQLite database created and migrated on startup | P0 |
| BE-03 | All token API endpoints return correct data | P0 |
| BE-04 | Hourly/daily aggregation runs correctly | P1 |
| BE-05 | Quota-aware routing respects budgets | P1 |
| BE-06 | Cost calculation matches configured pricing | P1 |
| BE-07 | WebSocket events sent for token updates | P2 |
| BE-08 | Data export (CSV/JSON) works correctly | P2 |
| BE-09 | Retention policy deletes old data | P3 |

### 7.2 Frontend

| ID | Criteria | Priority |
|----|----------|----------|
| FE-01 | Tokens page displays summary and charts | P0 |
| FE-02 | Time range selector updates all components | P0 |
| FE-03 | Quotas page allows viewing/editing quotas | P1 |
| FE-04 | Pricing page allows configuring model prices | P1 |
| FE-05 | Overview page shows token summary | P1 |
| FE-06 | Logs page shows token columns | P2 |
| FE-07 | Export functionality downloads correct files | P2 |
| FE-08 | Real-time updates via WebSocket | P2 |
| FE-09 | Responsive design works on tablet | P3 |

### 7.3 Performance

| ID | Criteria | Threshold |
|----|----------|-----------|
| PF-01 | Token summary API response time | < 100ms |
| PF-02 | Time-series query (90 days, hour granularity) | < 200ms |
| PF-03 | Log insert with token data | < 10ms |
| PF-04 | UI initial render | < 500ms |
| PF-05 | UI time range switch | < 300ms |
| PF-06 | Database size for 90 days @ 10k req/day | < 500MB |

---

## 8. Implementation Phases

### Phase 1: Core Infrastructure (Week 1)

**Goal**: Token extraction and persistence working

- [ ] Add token fields to LogEntry
- [ ] Implement token_extractor.rs
- [ ] Create storage module with SQLite
- [ ] Update proxy.rs to extract and store tokens
- [ ] Unit tests for extraction

**Verification**: Run proxy, make request, verify tokens stored in database

### Phase 2: API Layer (Week 2)

**Goal**: All token APIs functional

- [ ] Implement /zz/api/tokens/* endpoints
- [ ] Implement aggregation background task
- [ ] Add WebSocket token events
- [ ] API integration tests

**Verification**: curl endpoints return correct data

### Phase 3: UI - Analytics (Week 3)

**Goal**: Token analytics page complete

- [ ] Create TokensPage component
- [ ] Implement charts (Recharts)
- [ ] Time range selector
- [ ] Export functionality
- [ ] E2E tests

**Verification**: Navigate UI, see real token data

### Phase 4: UI - Quotas (Week 4)

**Goal**: Quota management complete

- [ ] Create QuotasPage component
- [ ] Create PricingPage component
- [ ] Update Overview page
- [ ] Update Logs page
- [ ] E2E tests

**Verification**: Configure quotas, see them respected in routing

### Phase 5: Integration & Polish (Week 5)

**Goal**: Production ready

- [ ] Quota-aware routing integration
- [ ] Error handling and edge cases
- [ ] Performance optimization
- [ ] Documentation
- [ ] Full test coverage

**Verification**: Complete test pass, performance benchmarks met

---

## 9. Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Token extraction fails for some providers | High | Medium | Fallback to N/A, log warning |
| SQLite performance degrades with large data | Medium | Low | Pre-aggregation tables, indexes, WAL mode |
| Cost calculation mismatch with actual billing | High | Medium | Document as estimate, allow user override |
| Quota reset fails silently | High | Low | Logging, alerting, manual reset API |
| UI performance with large datasets | Medium | Medium | Virtualization, pagination, aggregation |

---

## 10. Open Questions

1. **Streaming token extraction**: For SSE responses, usage may be in final chunk. Should we buffer chunks or accept missing data?
   - *Recommendation*: Accept missing data for streaming, document limitation

2. **Multi-currency support**: Should we support currencies other than USD?
   - *Recommendation*: V1 USD only, add currency config in V2

3. **Historical data import**: Should we support importing historical data?
   - *Recommendation*: Out of scope for V1

4. **Real-time alerts**: Should we support webhooks/notifications for quota alerts?
   - *Recommendation*: V1 WebSocket only, webhooks in V2

---

## 11. Appendix

### A. API Response Examples

Full examples for each endpoint are in `docs/api-examples.md`

### B. Database Query Examples

```sql
-- Today's token usage
SELECT 
    SUM(total_tokens) as total,
    SUM(input_tokens) as input,
    SUM(output_tokens) as output,
    SUM(cost_usd) as cost
FROM request_logs
WHERE date(timestamp) = date('now');

-- Top 5 models by cost this month
SELECT model, SUM(cost_usd) as total_cost
FROM request_logs
WHERE strftime('%Y-%m', timestamp) = strftime('%Y-%m', 'now')
GROUP BY model
ORDER BY total_cost DESC
LIMIT 5;

-- Provider quota status
SELECT 
    q.provider,
    q.monthly_token_budget,
    COALESCE(u.tokens_used, 0) as tokens_used,
    CAST(COALESCE(u.tokens_used, 0) AS REAL) / q.monthly_token_budget * 100 as usage_pct
FROM provider_quotas q
LEFT JOIN current_usage u ON q.provider = u.provider;
```

### C. Default Model Pricing

| Model | Input ($/1k) | Output ($/1k) |
|-------|--------------|---------------|
| claude-3-opus | 0.015 | 0.075 |
| claude-3-sonnet | 0.003 | 0.015 |
| claude-3-haiku | 0.00025 | 0.00125 |
| gpt-4 | 0.03 | 0.06 |
| gpt-4-turbo | 0.01 | 0.03 |
| gpt-3.5-turbo | 0.0005 | 0.0015 |
| qwen-plus | 0.0004 | 0.0012 |
| qwen-turbo | 0.0002 | 0.0006 |
| glm-4 | 0.0014 | 0.0014 |
| glm-4-flash | 0.0001 | 0.0001 |

---

**Document Status**: Ready for Implementation
**Next Review**: After Phase 1 completion