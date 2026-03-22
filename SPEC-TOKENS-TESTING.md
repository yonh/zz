# ZZ - Token Statistics Testing Strategy

## Version: 1.0.0

---

## 1. Testing Pyramid

```
           ┌─────────┐
           │   E2E   │  ← 5% (Critical user flows)
           │  Tests  │
           ├─────────┤
         ┌─┴─────────┴─┐
         │ Integration │  ← 25% (API + DB interactions)
         │    Tests    │
         ├─────────────┤
       ┌─┴─────────────┴─┐
       │    Unit Tests   │  ← 70% (Pure functions, components)
       └─────────────────┘
```

---

## 2. Backend Unit Tests

### 2.1 Token Extractor Tests

**File**: `src/token_extractor.rs` (tests module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    mod extract_usage {
        use super::*;
        
        #[test]
        fn openai_format_success() {
            let body = br#"{
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "choices": [],
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 50,
                    "total_tokens": 150
                }
            }"#;
            
            let usage = extract_usage(body, None).unwrap();
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 50);
            assert_eq!(usage.total_tokens, 150);
            assert!(usage.cached_tokens.is_none());
        }
        
        #[test]
        fn openai_format_with_cached_tokens() {
            let body = br#"{
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 50,
                    "total_tokens": 150,
                    "cached_tokens": 20
                }
            }"#;
            
            let usage = extract_usage(body, None).unwrap();
            assert_eq!(usage.cached_tokens, Some(20));
        }
        
        #[test]
        fn dashscope_format() {
            let body = br#"{
                "output": {
                    "choices": [],
                    "usage": {
                        "prompt_tokens": 100,
                        "output_tokens": 50,
                        "total_tokens": 150
                    }
                }
            }"#;
            
            let usage = extract_usage(body, Some("dashscope")).unwrap();
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 50);
        }
        
        #[test]
        fn anthropic_format() {
            let body = br#"{
                "id": "msg_123",
                "type": "message",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "cache_read_tokens": 30,
                    "cache_creation_tokens": 10
                }
            }"#;
            
            let usage = extract_usage(body, Some("anthropic")).unwrap();
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 50);
            assert_eq!(usage.cached_tokens, Some(30));
        }
        
        #[test]
        fn missing_usage_returns_none() {
            let body = br#"{"id": "chatcmpl-123", "choices": []}"#;
            assert!(extract_usage(body, None).is_none());
        }
        
        #[test]
        fn malformed_json_returns_none() {
            let body = b"not valid json {{{";
            assert!(extract_usage(body, None).is_none());
        }
        
        #[test]
        fn empty_body_returns_none() {
            let body = b"";
            assert!(extract_usage(body, None).is_none());
        }
        
        #[test]
        fn partial_usage_fields() {
            // Only prompt_tokens, no completion_tokens
            let body = br#"{
                "usage": {
                    "prompt_tokens": 100
                }
            }"#;
            
            // Should fail because completion_tokens is required
            assert!(extract_usage(body, None).is_none());
        }
        
        #[test]
        fn zero_tokens_valid() {
            let body = br#"{
                "usage": {
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0
                }
            }"#;
            
            let usage = extract_usage(body, None).unwrap();
            assert_eq!(usage.input_tokens, 0);
            assert_eq!(usage.output_tokens, 0);
        }
        
        #[test]
        fn large_token_counts() {
            let body = br#"{
                "usage": {
                    "prompt_tokens": 1000000,
                    "completion_tokens": 500000,
                    "total_tokens": 1500000
                }
            }"#;
            
            let usage = extract_usage(body, None).unwrap();
            assert_eq!(usage.input_tokens, 1000000);
            assert_eq!(usage.total_tokens, 1500000);
        }
    }
    
    mod extract_usage_from_chunk {
        use super::*;
        
        #[test]
        fn sse_data_chunk_with_usage() {
            let chunk = r#"data: {"usage": {"prompt_tokens": 100, "completion_tokens": 50}}"#;
            
            match extract_usage_from_chunk(chunk) {
                SseChunkType::Usage(usage) => {
                    assert_eq!(usage.input_tokens, 100);
                    assert_eq!(usage.output_tokens, 50);
                }
                _ => panic!("Expected Usage type"),
            }
        }
        
        #[test]
        fn sse_done_chunk() {
            let chunk = "data: [DONE]";
            assert!(matches!(extract_usage_from_chunk(chunk), SseChunkType::Done));
        }
        
        #[test]
        fn sse_content_chunk() {
            let chunk = r#"data: {"choices": [{"delta": {"content": "Hello"}}]}"#;
            assert!(matches!(extract_usage_from_chunk(chunk), SseChunkType::Content));
        }
        
        #[test]
        fn sse_empty_chunk() {
            let chunk = "";
            assert!(matches!(extract_usage_from_chunk(chunk), SseChunkType::Unknown));
        }
        
        #[test]
        fn sse_anthropic_final_chunk() {
            let chunk = r#"data: {"type": "message_delta", "usage": {"output_tokens": 50}}"#;
            
            match extract_usage_from_chunk(chunk) {
                SseChunkType::Usage(usage) => {
                    assert_eq!(usage.output_tokens, 50);
                }
                _ => panic!("Expected Usage type"),
            }
        }
    }
}
```

### 2.2 Pricing Calculator Tests

**File**: `src/pricing.rs` (tests module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    fn test_pricing_config() -> PricingConfig {
        let mut prices = HashMap::new();
        prices.insert("gpt-4".to_string(), (0.03, 0.06));
        prices.insert("gpt-4*".to_string(), (0.03, 0.06));
        prices.insert("claude-3-opus".to_string(), (0.015, 0.075));
        prices.insert("*-turbo".to_string(), (0.01, 0.03));
        
        PricingConfig {
            prices,
            default_input: 0.001,
            default_output: 0.002,
        }
    }
    
    #[test]
    fn exact_match() {
        let config = test_pricing_config();
        let cost = config.calculate_cost("gpt-4", 1000, 500);
        // 1k * 0.03 + 0.5k * 0.06 = 0.03 + 0.03 = 0.06
        assert!((cost - 0.06).abs() < 0.0001);
    }
    
    #[test]
    fn prefix_wildcard_match() {
        let config = test_pricing_config();
        let cost = config.calculate_cost("gpt-4-turbo", 1000, 500);
        // Uses gpt-4* pattern
        assert!((cost - 0.06).abs() < 0.0001);
    }
    
    #[test]
    fn suffix_wildcard_match() {
        let config = test_pricing_config();
        let cost = config.calculate_cost("gpt-3.5-turbo", 1000, 500);
        // Uses *-turbo pattern
        // 1k * 0.01 + 0.5k * 0.03 = 0.01 + 0.015 = 0.025
        assert!((cost - 0.025).abs() < 0.0001);
    }
    
    #[test]
    fn no_match_uses_default() {
        let config = test_pricing_config();
        let cost = config.calculate_cost("unknown-model", 1000, 1000);
        // 1k * 0.001 + 1k * 0.002 = 0.001 + 0.002 = 0.003
        assert!((cost - 0.003).abs() < 0.0001);
    }
    
    #[test]
    fn zero_tokens_zero_cost() {
        let config = test_pricing_config();
        let cost = config.calculate_cost("gpt-4", 0, 0);
        assert!((cost - 0.0).abs() < 0.0001);
    }
    
    #[test]
    fn fractional_tokens() {
        let config = test_pricing_config();
        // 500 tokens = 0.5k
        let cost = config.calculate_cost("gpt-4", 500, 250);
        // 0.5k * 0.03 + 0.25k * 0.06 = 0.015 + 0.015 = 0.03
        assert!((cost - 0.03).abs() < 0.0001);
    }
    
    #[test]
    fn large_token_count() {
        let config = test_pricing_config();
        // 1M tokens
        let cost = config.calculate_cost("gpt-4", 1_000_000, 500_000);
        // 1000k * 0.03 + 500k * 0.06 = 30 + 30 = 60
        assert!((cost - 60.0).abs() < 0.01);
    }
    
    #[test]
    fn pattern_priority_exact_over_wildcard() {
        let mut prices = HashMap::new();
        prices.insert("gpt-4".to_string(), (0.05, 0.10));  // More expensive
        prices.insert("gpt-*".to_string(), (0.01, 0.02));  // Cheaper
        
        let config = PricingConfig {
            prices,
            default_input: 0.001,
            default_output: 0.002,
        };
        
        // Exact match should be preferred
        let cost = config.calculate_cost("gpt-4", 1000, 1000);
        // Uses exact: 1k * 0.05 + 1k * 0.10 = 0.15
        assert!((cost - 0.15).abs() < 0.0001);
    }
    
    #[test]
    fn cached_tokens_deduction() {
        let config = test_pricing_config();
        let cost = config.calculate_cost_with_cache("gpt-4", 1000, 500, Some(200));
        // Without cache: 0.06
        // With 200 cached: 0.2k * 0.03 * 0.9 = 0.0054 savings
        // Total: 0.06 - 0.0054 = 0.0546
        assert!((cost - 0.0546).abs() < 0.0001);
    }
}
```

### 2.3 Storage Layer Tests

**File**: `src/storage/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    struct TestContext {
        storage: TokenStorage,
        _db_file: NamedTempFile,
    }
    
    impl TestContext {
        fn new() -> Self {
            let db_file = NamedTempFile::new().unwrap();
            let storage = TokenStorage::new(db_file.path().to_str().unwrap()).unwrap();
            Self { storage, _db_file: db_file }
        }
    }
    
    fn sample_log_entry(id: &str) -> LogEntry {
        LogEntry {
            id: id.to_string(),
            timestamp: "2026-03-22T10:00:00Z".to_string(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            provider: "test-provider".to_string(),
            status: 200,
            duration_ms: 1000,
            ttfb_ms: 500,
            model: "gpt-4".to_string(),
            streaming: false,
            request_bytes: 100,
            response_bytes: 200,
            failover_chain: None,
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: Some(150),
            cached_tokens: None,
            cost_usd: Some(0.015),
        }
    }
    
    #[test]
    fn insert_and_retrieve_log() {
        let ctx = TestContext::new();
        let entry = sample_log_entry("req_001");
        
        ctx.storage.insert_log(&entry).unwrap();
        
        let retrieved = ctx.storage.get_log_by_id("req_001").unwrap();
        assert_eq!(rerieved.id, "req_001");
        assert_eq!(retrieved.input_tokens, Some(100));
    }
    
    #[test]
    fn query_timeseries_hourly() {
        let ctx = TestContext::new();
        
        // Insert logs across multiple hours
        for hour in 0..3 {
            for i in 0..5 {
                let mut entry = sample_log_entry(&format!("req_{}{}", hour, i));
                entry.timestamp = format!("2026-03-22T{}0:00:00Z", hour + 10);
                entry.input_tokens = Some(100 * (hour + 1));
                ctx.storage.insert_log(&entry).unwrap();
            }
        }
        
        let result = ctx.storage.query_timeseries(
            "2026-03-22T10:00:00Z",
            "2026-03-22T13:00:00Z",
            "hour",
            None,
            None,
        ).unwrap();
        
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].input_tokens, 500);  // 5 logs * 100 each
        assert_eq!(result[1].input_tokens, 1000); // 5 logs * 200 each
    }
    
    #[test]
    fn quota_crud_operations() {
        let ctx = TestContext::new();
        
        // Create
        ctx.storage.set_quota(&QuotaConfig {
            provider: "test-provider".to_string(),
            monthly_token_budget: Some(1000000),
            monthly_cost_budget_usd: Some(100.0),
            alert_threshold: 0.8,
            reset_day: 1,
        }).unwrap();
        
        // Read
        let quota = ctx.storage.get_quota("test-provider").unwrap().unwrap();
        assert_eq!(quota.monthly_token_budget, Some(1000000));
        
        // Update
        ctx.storage.set_quota(&QuotaConfig {
            provider: "test-provider".to_string(),
            monthly_token_budget: Some(2000000),
            monthly_cost_budget_usd: Some(200.0),
            alert_threshold: 0.9,
            reset_day: 15,
        }).unwrap();
        
        let updated = ctx.storage.get_quota("test-provider").unwrap().unwrap();
        assert_eq!(updated.monthly_token_budget, Some(2000000));
        assert_eq!(updated.reset_day, 15);
        
        // Delete
        ctx.storage.delete_quota("test-provider").unwrap();
        assert!(ctx.storage.get_quota("test-provider").unwrap().is_none());
    }
    
    #[test]
    fn usage_tracking_and_reset() {
        let ctx = TestContext::new();
        
        ctx.storage.set_quota(&QuotaConfig {
            provider: "test-provider".to_string(),
            monthly_token_budget: Some(1000000),
            monthly_cost_budget_usd: Some(100.0),
            alert_threshold: 0.8,
            reset_day: 1,
        }).unwrap();
        
        // Increment usage
        ctx.storage.increment_usage("test-provider", 1000, 0.10).unwrap();
        ctx.storage.increment_usage("test-provider", 500, 0.05).unwrap();
        
        let usage = ctx.storage.get_current_usage("test-provider").unwrap();
        assert_eq!(usage.tokens_used, 1500);
        assert!((usage.cost_used_usd - 0.15).abs() < 0.001);
        
        // Reset
        ctx.storage.reset_usage("test-provider").unwrap();
        
        let after_reset = ctx.storage.get_current_usage("test-provider").unwrap();
        assert_eq!(after_reset.tokens_used, 0);
    }
    
    #[test]
    fn batch_insert_performance() {
        let ctx = TestContext::new();
        
        let entries: Vec<LogEntry> = (0..100)
            .map(|i| sample_log_entry(&format!("req_{:03}", i)))
            .collect();
        
        let start = std::time::Instant::now();
        ctx.storage.insert_logs_batch(&entries).unwrap();
        let duration = start.elapsed();
        
        // Should be under 50ms for 100 entries
        assert!(duration.as_millis() < 50);
    }
    
    #[test]
    fn concurrent_access() {
        let ctx = std::sync::Arc::new(TestContext::new());
        let mut handles = vec![];
        
        for thread_id in 0..4 {
            let ctx_clone = ctx.clone();
            let handle = std::thread::spawn(move || {
                for i in 0..25 {
                    let entry = sample_log_entry(&format!("t{}i{}", thread_id, i));
                    ctx_clone.storage.insert_log(&entry).unwrap();
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // All 100 entries should be present
        let count = ctx.storage.count_logs().unwrap();
        assert_eq!(count, 100);
    }
}
```

---

## 3. Integration Tests

### 3.1 API Integration Tests

**File**: `tests/token_api_integration.rs`

```rust
use zz::test_utils::{spawn_app, TestApp};

#[tokio::test]
async fn token_summary_returns_valid_structure() {
    let app = spawn_app().await;
    
    let response = app.get("/zz/api/tokens/summary").await;
    assert_eq!(response.status(), 200);
    
    let body: serde_json::Value = response.json().await;
    
    // Check structure
    assert!(body.get("today").is_some());
    assert!(body.get("yesterday").is_some());
    assert!(body.get("thisWeek").is_some());
    assert!(body.get("thisMonth").is_some());
    assert!(body.get("lastMonth").is_some());
    
    // Check today's structure
    let today = &body["today"];
    assert!(today.get("totalTokens").is_some());
    assert!(today.get("inputTokens").is_some());
    assert!(today.get("outputTokens").is_some());
    assert!(today.get("totalCostUsd").is_some());
}

#[tokio::test]
async fn timeseries_query_with_valid_params() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/timeseries")
        .query("start", "2026-03-01T00:00:00Z")
        .query("end", "2026-03-22T00:00:00Z")
        .query("granularity", "hour")
        .await;
    
    assert_eq!(response.status(), 200);
    
    let body: serde_json::Value = response.json().await;
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn timeseries_invalid_granularity_returns_400() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/timeseries")
        .query("start", "2026-03-01T00:00:00Z")
        .query("end", "2026-03-22T00:00:00Z")
        .query("granularity", "invalid")
        .await;
    
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn timeseries_invalid_time_range_returns_400() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/timeseries")
        .query("start", "2026-03-22T00:00:00Z")
        .query("end", "2026-03-01T00:00:00Z")  // End before start
        .await;
    
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn quota_create_update_delete() {
    let app = spawn_app().await;
    
    // Create provider first
    let _ = app.post("/zz/api/providers")
        .json(&serde_json::json!({
            "name": "test-provider",
            "base_url": "https://api.test.com",
            "api_key": "test-key"
        }))
        .await;
    
    // Create quota
    let create_response = app.put("/zz/api/quotas")
        .json(&serde_json::json!({
            "quotas": [{
                "provider": "test-provider",
                "monthlyTokenBudget": 1000000,
                "monthlyCostBudgetUsd": 100.0,
                "alertThreshold": 0.8,
                "resetDay": 1
            }]
        }))
        .await;
    
    assert_eq!(create_response.status(), 200);
    
    // Read quota
    let read_response = app.get("/zz/api/quotas").await;
    assert_eq!(read_response.status(), 200);
    
    let body: serde_json::Value = read_response.json().await;
    let quota = body["quotas"].as_array().unwrap().iter()
        .find(|q| q["provider"] == "test-provider")
        .unwrap();
    
    assert_eq!(quota["monthlyTokenBudget"], 1000000);
    
    // Update quota
    let update_response = app.put("/zz/api/quotas")
        .json(&serde_json::json!({
            "quotas": [{
                "provider": "test-provider",
                "monthlyTokenBudget": 2000000,
                "monthlyCostBudgetUsd": 200.0,
                "alertThreshold": 0.9,
                "resetDay": 15
            }]
        }))
        .await;
    
    assert_eq!(update_response.status(), 200);
    
    // Verify update
    let verify_response = app.get("/zz/api/quotas").await;
    let verify_body: serde_json::Value = verify_response.json().await;
    let updated = verify_body["quotas"].as_array().unwrap().iter()
        .find(|q| q["provider"] == "test-provider")
        .unwrap();
    
    assert_eq!(updated["monthlyTokenBudget"], 2000000);
    assert_eq!(updated["resetDay"], 15);
    
    // Delete quota
    let delete_response = app.delete("/zz/api/quotas/test-provider").await;
    assert_eq!(delete_response.status(), 200);
}

#[tokio::test]
async fn pricing_crud() {
    let app = spawn_app().await;
    
    // Set pricing
    let set_response = app.put("/zz/api/pricing")
        .json(&serde_json::json!({
            "defaultPricing": {
                "inputPricePer1k": 0.001,
                "outputPricePer1k": 0.002
            },
            "modelPricing": [
                {
                    "modelPattern": "gpt-4",
                    "inputPricePer1k": 0.03,
                    "outputPricePer1k": 0.06
                }
            ]
        }))
        .await;
    
    assert_eq!(set_response.status(), 200);
    
    // Get pricing
    let get_response = app.get("/zz/api/pricing").await;
    assert_eq!(get_response.status(), 200);
    
    let body: serde_json::Value = get_response.json().await;
    assert_eq!(body["defaultPricing"]["inputPricePer1k"], 0.001);
    assert_eq!(body["modelPricing"][0]["modelPattern"], "gpt-4");
}

#[tokio::test]
async fn export_csv_format() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/export")
        .query("format", "csv")
        .query("start", "2026-03-01T00:00:00Z")
        .query("end", "2026-03-22T00:00:00Z")
        .await;
    
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("content-type").unwrap().contains("text/csv"));
    
    let body = response.text().await;
    assert!(body.starts_with("timestamp,provider,model"));
}

#[tokio::test]
async fn export_json_format() {
    let app = spawn_app().await;
    
    let response = app
        .get("/zz/api/tokens/export")
        .query("format", "json")
        .query("start", "2026-03-01T00:00:00Z")
        .query("end", "2026-03-22T00:00:00Z")
        .await;
    
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("content-type").unwrap().contains("application/json"));
    
    let body: serde_json::Value = response.json().await;
    assert!(body.get("records").is_some());
}
```

### 3.2 Proxy Integration Tests

**File**: `tests/proxy_token_tracking.rs`

```rust
use zz::test_utils::{spawn_app, TestApp, mock_provider_server};

#[tokio::test]
async fn proxy_extracts_tokens_from_response() {
    let mut mock_server = mock_provider_server().await;
    
    // Mock response with token usage
    mock_server.mock(|when, then| {
        when.path("/v1/chat/completions");
        then.json_body(json!({
            "id": "chatcmpl-123",
            "choices": [{"message": {"content": "Hello"}}],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        }));
    });
    
    let app = spawn_app().await;
    
    // Make request through proxy
    let response = app.proxy_post("/v1/chat/completions")
        .json(&json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        }))
        .await;
    
    assert_eq!(response.status(), 200);
    
    // Verify token was stored
    let logs = app.get("/zz/api/logs").await.json().await;
    let log = &logs["logs"][0];
    
    assert_eq!(log["inputTokens"], 100);
    assert_eq!(log["outputTokens"], 50);
}

#[tokio::test]
async fn proxy_handles_streaming_with_tokens() {
    let mut mock_server = mock_provider_server().await;
    
    // Mock SSE response
    mock_server.mock(|when, then| {
        when.path("/v1/chat/completions")
            .header("accept", "text/event-stream");
        then.body(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
             data: {\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":50}}\n\n\
             data: [DONE]\n\n"
        ).header("content-type", "text/event-stream");
    });
    
    let app = spawn_app().await;
    
    let response = app.proxy_post("/v1/chat/completions")
        .header("accept", "text/event-stream")
        .json(&json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "stream": true
        }))
        .await;
    
    assert_eq!(response.status(), 200);
    
    // Wait for stream to complete and tokens to be extracted
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Verify token was extracted from stream
    let logs = app.get("/zz/api/logs").await.json().await;
    let log = &logs["logs"][0];
    
    assert_eq!(log["inputTokens"], 100);
    assert_eq!(log["outputTokens"], 50);
}
```

---

## 4. Frontend Tests

### 4.1 Component Tests

**File**: `ui/src/components/tokens/__tests__/StatsCard.test.tsx`

```typescript
import { render, screen } from '@testing-library/react';
import { StatsCard } from '../StatsCard';

describe('StatsCard', () => {
  it('renders title and value', () => {
    render(<StatsCard title="Total Tokens" value="1,234" />);
    
    expect(screen.getByText('Total Tokens')).toBeInTheDocument();
    expect(screen.getByText('1,234')).toBeInTheDocument();
  });
  
  it('shows upward trend', () => {
    render(
      <StatsCard
        title="Total Tokens"
        value="1,234"
        trend={{ value: '15', direction: 'up' }}
      />
    );
    
    expect(screen.getByText('15%')).toBeInTheDocument();
    expect(screen.getByText('15%')).toHaveClass('text-green-500');
  });
  
  it('shows downward trend', () => {
    render(
      <StatsCard
        title="Total Tokens"
        value="1,234"
        trend={{ value: '10', direction: 'down' }}
      />
    );
    
    expect(screen.getByText('10%')).toBeInTheDocument();
    expect(screen.getByText('10%')).toHaveClass('text-red-500');
  });
  
  it('shows loading skeleton', () => {
    render(<StatsCard title="Total Tokens" value="1,234" loading />);
    
    expect(screen.queryByText('Total Tokens')).not.toBeInTheDocument();
    // Check for skeleton elements
    expect(document.querySelector('.animate-pulse')).toBeInTheDocument();
  });
  
  it('renders without trend', () => {
    render(<StatsCard title="Total Tokens" value="1,234" />);
    
    expect(screen.queryByText('%')).not.toBeInTheDocument();
  });
});
```

### 4.2 Hook Tests

**File**: `ui/src/hooks/__tests__/useTokenQueries.test.ts`

```typescript
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useTokenSummary, useTimeSeries, useQuotas } from '../useTokenQueries';

// Mock fetch
global.fetch = jest.fn();

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
};

describe('useTokenSummary', () => {
  beforeEach(() => {
    (fetch as jest.Mock).mockClear();
  });
  
  it('fetches and returns summary data', async () => {
    const mockData = {
      today: { totalTokens: 1000, inputTokens: 700, outputTokens: 300 },
      yesterday: { totalTokens: 800, inputTokens: 500, outputTokens: 300 },
    };
    
    (fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockData),
    });
    
    const { result } = renderHook(() => useTokenSummary(), {
      wrapper: createWrapper(),
    });
    
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    
    expect(result.current.data).toEqual(mockData);
    expect(fetch).toHaveBeenCalledWith('/zz/api/tokens/summary');
  });
  
  it('handles fetch error', async () => {
    (fetch as jest.Mock).mockResolvedValueOnce({
      ok: false,
      status: 500,
    });
    
    const { result } = renderHook(() => useTokenSummary(), {
      wrapper: createWrapper(),
    });
    
    await waitFor(() => expect(result.current.isError).toBe(true));
    
    expect(result.current.error).toBeDefined();
  });
});

describe('useTimeSeries', () => {
  it('constructs correct query params', async () => {
    const mockData = { data: [], start: '2026-03-01', end: '2026-03-22' };
    
    (fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockData),
    });
    
    const { result } = renderHook(
      () => useTimeSeries({
        start: '2026-03-01T00:00:00Z',
        end: '2026-03-22T00:00:00Z',
        granularity: 'hour',
        provider: 'test-provider',
      }),
      { wrapper: createWrapper() }
    );
    
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('/zz/api/tokens/timeseries?')
    );
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('start=2026-03-01T00%3A00%3A00Z')
    );
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('provider=test-provider')
    );
  });
});

describe('useQuotas', () => {
  it('fetches quota data', async () => {
    const mockData = {
      quotas: [
        {
          provider: 'test-provider',
          monthlyTokenBudget: 1000000,
          currentUsage: { tokensUsed: 500000 },
        },
      ],
    };
    
    (fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(mockData),
    });
    
    const { result } = renderHook(() => useQuotas(), {
      wrapper: createWrapper(),
    });
    
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    
    expect(result.current.data?.quotas).toHaveLength(1);
    expect(result.current.data?.quotas[0].provider).toBe('test-provider');
  });
});
```

---

## 5. Test Coverage Targets

| Layer | Target Coverage | Critical Paths |
|-------|----------------|----------------|
| Token Extractor | 95% | All provider formats, edge cases |
| Pricing Calculator | 95% | Pattern matching, cost calculation |
| Storage Layer | 90% | CRUD, queries, concurrency |
| API Handlers | 85% | All endpoints, error handling |
| Proxy Integration | 80% | Token extraction, streaming |
| Frontend Components | 75% | Rendering, user interactions |
| Frontend Hooks | 85% | Data fetching, caching |

---

## 6. CI/CD Test Commands

```yaml
# .github/workflows/test.yml
jobs:
  test-backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - name: Run unit tests
        run: cargo test --lib
      - name: Run integration tests
        run: cargo test --test '*_integration'
      - name: Generate coverage
        run: cargo llvm-cov --html

  test-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - name: Install dependencies
        run: cd ui && npm ci
      - name: Run tests
        run: cd ui && npm test -- --coverage
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

---

**Document Version**: 1.0
**Last Updated**: 2026-03-22