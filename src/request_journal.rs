//! Request Journal - Persistent storage for LLM request forensics.
//!
//! Captures complete request details (headers, body) for debugging
//! parameter compatibility and upstream behavior issues.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;

/// Timing breakdown for a request.
///
/// Records key step durations and routing decision context for debugging.
/// All time values are in milliseconds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RequestTiming {
    /// Model parsing duration — extract_model + get_available_for_model
    pub parse_model_ms: u64,
    /// Provider selection duration — cumulative across all retry rounds
    pub select_provider_ms: u64,
    /// Upstream request total duration — cumulative across all attempts
    /// SSE: from request start to last chunk arriving at proxy
    /// Non-SSE: complete HTTP round-trip
    pub upstream_total_ms: u64,
    /// Upstream time-to-first-byte — final successful attempt's TTFB
    /// SSE: first chunk from upstream arriving at proxy
    pub upstream_ttfb_ms: u64,
    /// Number of failed attempts before success (0 = first attempt succeeded)
    pub retry_count: u8,
    /// Provider name for each attempt (successful or failed)
    /// On success: length = retry_count + 1 (N failed + 1 successful)
    /// On all-failed: length = retry_count (N failed, no successful attempt)
    pub retry_providers: Vec<String>,
    /// Wall-clock duration (ms) for each attempt — same length as retry_providers
    pub retry_durations_ms: Vec<u64>,
    /// Number of available candidate providers
    pub available_providers: u16,
    /// Routing decision reason — prefixed: pinned:|rule:|strategy:
    /// Max 128 bytes
    pub selection_reason: String,
    /// Whether timing data is complete (false = partial, some fields are default 0)
    pub completed: bool,
}

/// A single entry in the request journal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestJournalEntry {
    pub id: String,
    pub timestamp: String,
    pub client_name: String,
    pub user_agent: String,
    pub method: String,
    pub path: String,
    pub provider: String,
    pub upstream_url: String,
    pub model: String,
    pub streaming: bool,
    pub status: u16,
    pub request_headers: HashMap<String, String>,
    pub request_content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body_base64: Option<String>,
    pub request_bytes: u64,
    pub response_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failover_chain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<RequestTiming>,
}

/// Timing summary embedded in list view entries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimingSummary {
    pub upstream_total_ms: u64,
    pub upstream_ttfb_ms: u64,
    pub completed: bool,
}

/// Summary of a journal entry for list view (excludes body).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestJournalSummary {
    pub id: String,
    pub timestamp: String,
    pub client_name: String,
    pub user_agent: String,
    pub method: String,
    pub path: String,
    pub provider: String,
    pub model: String,
    pub streaming: bool,
    pub status: u16,
    pub request_bytes: u64,
    pub response_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing_summary: Option<TimingSummary>,
}

impl From<RequestJournalEntry> for RequestJournalSummary {
    fn from(entry: RequestJournalEntry) -> Self {
        Self {
            id: entry.id,
            timestamp: entry.timestamp,
            client_name: entry.client_name,
            user_agent: entry.user_agent,
            method: entry.method,
            path: entry.path,
            provider: entry.provider,
            model: entry.model,
            streaming: entry.streaming,
            status: entry.status,
            request_bytes: entry.request_bytes,
            response_bytes: entry.response_bytes,
            timing_summary: entry.timing.as_ref().map(|t| TimingSummary {
                upstream_total_ms: t.upstream_total_ms,
                upstream_ttfb_ms: t.upstream_ttfb_ms,
                completed: t.completed,
            }),
        }
    }
}

/// Query filters for listing journal entries.
#[derive(Debug, Clone, Default)]
pub struct JournalQuery {
    pub client: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<u16>,
    pub path: Option<String>,
    pub date: Option<String>,
    pub failed_only: bool,
    pub slow_only: bool,
}

/// Writer for request journal entries.
///
/// Stores each request as a separate JSON file organized by date:
/// ```text
/// logs/request-journal/
///   2026-03-26/
///     req_abc123.json
///     req_def456.json
/// ```
pub struct RequestJournalWriter {
    config: std::sync::RwLock<crate::config::RequestJournalConfig>,
    created_dirs: Arc<AsyncRwLock<std::collections::HashSet<String>>>,
}

impl RequestJournalWriter {
    pub fn new(config: crate::config::RequestJournalConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            created_dirs: Arc::new(AsyncRwLock::new(std::collections::HashSet::new())),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    pub fn storage_dir(&self) -> String {
        self.config.read().unwrap().storage_dir.clone()
    }

    pub async fn write_entry(&self, entry: RequestJournalEntry) {
        let (enabled, storage_dir) = {
            let config = self.config.read().unwrap();
            (config.enabled, config.storage_dir.clone())
        };
        
        if !enabled {
            return;
        }

        let date = if entry.timestamp.len() >= 10 {
            &entry.timestamp[..10]
        } else {
            tracing::error!(
                id = %entry.id,
                timestamp = %entry.timestamp,
                "Invalid timestamp format, skipping journal entry"
            );
            return;
        };
        let date_dir = PathBuf::from(&storage_dir).join(date);
        
        {
            let mut created_dirs = self.created_dirs.write().await;
            if !created_dirs.contains(date) {
                if let Err(e) = tokio::fs::create_dir_all(&date_dir).await {
                    tracing::error!(
                        path = ?date_dir,
                        error = %e,
                        "Failed to create request journal directory"
                    );
                    return;
                }
                created_dirs.insert(date.to_string());
            }
        }

        let file_path = date_dir.join(format!("{}.json", entry.id));
        
        match serde_json::to_string_pretty(&entry) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&file_path, json).await {
                    tracing::error!(
                        path = ?file_path,
                        error = %e,
                        "Failed to write request journal entry"
                    );
                } else {
                    tracing::debug!(
                        id = %entry.id,
                        path = ?file_path,
                        "Wrote request journal entry"
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    id = %entry.id,
                    error = %e,
                    "Failed to serialize request journal entry"
                );
            }
        }
    }

    pub fn redact_headers(&self, headers: &hyper::HeaderMap) -> HashMap<String, String> {
        let config = self.config.read().unwrap();
        let mut result = HashMap::new();
        
        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();
            let value_str = if config.redact_headers.iter().any(|h| h.eq_ignore_ascii_case(&name_str)) {
                "[REDACTED]".to_string()
            } else if let Ok(v) = value.to_str() {
                v.to_string()
            } else {
                String::from_utf8_lossy(value.as_bytes()).to_string()
            };
            
            result.insert(name_str, value_str);
        }
        
        result
    }

    pub fn update_config(&self, new_config: crate::config::RequestJournalConfig) {
        let mut config = self.config.write().unwrap();
        *config = new_config;
    }
}

pub async fn list_entries(
    storage_dir: &str,
    query: JournalQuery,
    offset: usize,
    limit: usize,
) -> Result<(Vec<RequestJournalSummary>, usize), String> {
    let base_path = PathBuf::from(storage_dir);
    
    if !base_path.exists() {
        return Ok((Vec::new(), 0));
    }

    let mut entries: Vec<RequestJournalEntry> = Vec::new();
    
    let date_dirs: Vec<String> = if let Some(ref date) = query.date {
        if date.contains("..") || date.contains('/') || date.contains('\\') {
            return Err("Invalid date filter".to_string());
        }
        vec![date.clone()]
    } else {
        match tokio::fs::read_dir(&base_path).await {
            Ok(mut dirs) => {
                let mut date_dirs = Vec::new();
                while let Ok(Some(entry)) = dirs.next_entry().await {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.len() == 10 && name.contains('-') {
                                date_dirs.push(name.to_string());
                            }
                        }
                    }
                }
                date_dirs.sort();
                date_dirs.reverse();
                date_dirs
            }
            Err(e) => return Err(format!("Failed to read storage directory: {}", e)),
        }
    };

    for date_dir in date_dirs {
        let date_path = base_path.join(&date_dir);
        
        let mut files = match tokio::fs::read_dir(&date_path).await {
            Ok(files) => files,
            Err(_) => continue,
        };

        while let Ok(Some(file)) = files.next_entry().await {
            let path = file.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        match serde_json::from_str::<RequestJournalEntry>(&content) {
                            Ok(entry) => {
                                if matches_query(&entry, &query) {
                                    entries.push(entry);
                                }
                            }
                            Err(e) => {
                                tracing::warn!(path = ?path, error = %e, "Failed to parse journal entry");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to read journal entry");
                    }
                }
            }
        }
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    let total = entries.len();
    let summaries: Vec<RequestJournalSummary> = entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(RequestJournalSummary::from)
        .collect();

    Ok((summaries, total))
}

fn matches_query(entry: &RequestJournalEntry, query: &JournalQuery) -> bool {
    if let Some(ref client) = query.client {
        if !entry.client_name.eq_ignore_ascii_case(client) {
            return false;
        }
    }
    
    if let Some(ref provider) = query.provider {
        if !entry.provider.eq_ignore_ascii_case(provider) {
            return false;
        }
    }
    
    if let Some(ref model) = query.model {
        if !entry.model.contains(model) && !model.contains(&entry.model) {
            return false;
        }
    }
    
    if let Some(status) = query.status {
        if entry.status != status {
            return false;
        }
    }
    
    if let Some(ref path_filter) = query.path {
        if !entry.path.contains(path_filter) {
            return false;
        }
    }

    if query.failed_only {
        if entry.timing.as_ref().map_or(true, |t| t.completed) && entry.status < 400 {
            return false;
        }
    }

    if query.slow_only {
        if !entry.timing.as_ref().map_or(false, |t| t.upstream_total_ms > 3000) {
            return false;
        }
    }

    true
}

/// Validate that an entry ID is safe to use in filesystem paths.
/// Rejects IDs containing path traversal sequences or path separators.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("..")
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains(std::path::MAIN_SEPARATOR)
}

pub async fn get_entry(storage_dir: &str, id: &str) -> Result<Option<RequestJournalEntry>, String> {
    if !is_safe_id(id) {
        return Err(format!("Invalid entry ID: {}", id));
    }
    let base_path = PathBuf::from(storage_dir);
    
    if !base_path.exists() {
        return Ok(None);
    }

    let filename = format!("{}.json", id);
    
    let mut date_dirs = match tokio::fs::read_dir(&base_path).await {
        Ok(dirs) => dirs,
        Err(e) => return Err(format!("Failed to read storage directory: {}", e)),
    };

    while let Ok(Some(entry)) = date_dirs.next_entry().await {
        if entry.path().is_dir() {
            let file_path = entry.path().join(&filename);
            if file_path.exists() {
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(content) => {
                        match serde_json::from_str::<RequestJournalEntry>(&content) {
                            Ok(entry) => return Ok(Some(entry)),
                            Err(e) => return Err(format!("Failed to parse journal entry: {}", e)),
                        }
                    }
                    Err(e) => return Err(format!("Failed to read journal entry: {}", e)),
                }
            }
        }
    }

    Ok(None)
}

pub async fn export_entries(
    storage_dir: &str,
    query: JournalQuery,
) -> Result<Vec<RequestJournalEntry>, String> {
    let (summaries, _) = list_entries(storage_dir, query.clone(), 0, 10000).await?;
    
    let mut entries = Vec::new();
    for summary in summaries {
        if let Some(entry) = get_entry(storage_dir, &summary.id).await? {
            entries.push(entry);
        }
    }
    
    Ok(entries)
}

/// Infer client name from User-Agent header.
///
/// Recognizes common AI editor clients:
/// - Claude Code CLI
/// - OpenAI Codex
/// - Cursor
/// - Other OpenAI SDK clients
pub fn infer_client_name(user_agent: &str) -> String {
    let ua_lower = user_agent.to_lowercase();
    
    if ua_lower.contains("claude") || ua_lower.contains("claude-code") {
        return "claude".to_string();
    }
    
    if ua_lower.contains("cursor") {
        return "cursor".to_string();
    }
    
    if ua_lower.contains("codex") {
        return "codex".to_string();
    }
    
    if ua_lower.contains("openai") {
        return "openai-sdk".to_string();
    }
    
    if ua_lower.contains("anthropic") {
        return "anthropic-sdk".to_string();
    }
    
    if ua_lower.contains("vscode") || ua_lower.contains("vs code") {
        return "vscode".to_string();
    }
    
    if ua_lower.contains("python-requests") || ua_lower.contains("python/") {
        return "python".to_string();
    }
    
    if ua_lower.contains("node") || ua_lower.contains("axios") {
        return "nodejs".to_string();
    }
    
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_client_name_claude() {
        assert_eq!(infer_client_name("Claude-Code/1.0"), "claude");
        assert_eq!(infer_client_name("claude-code-cli"), "claude");
    }

    #[test]
    fn test_infer_client_name_cursor() {
        assert_eq!(infer_client_name("Cursor/0.40.0"), "cursor");
        assert_eq!(infer_client_name("cursor-ai"), "cursor");
    }

    #[test]
    fn test_infer_client_name_codex() {
        assert_eq!(infer_client_name("Codex/1.0"), "codex");
        assert_eq!(infer_client_name("openai-codex"), "codex");
    }

    #[test]
    fn test_infer_client_name_unknown() {
        assert_eq!(infer_client_name("Mozilla/5.0"), "unknown");
        assert_eq!(infer_client_name(""), "unknown");
    }

    #[test]
    fn test_redact_headers() {
        let config = crate::config::RequestJournalConfig::default();
        let writer = RequestJournalWriter::new(config);
        
        let mut headers = hyper::HeaderMap::new();
        headers.insert("authorization", "Bearer secret-token".parse().unwrap());
        headers.insert("x-api-key", "my-api-key".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("user-agent", "Claude-Code/1.0".parse().unwrap());
        
        let redacted = writer.redact_headers(&headers);
        
        assert_eq!(redacted.get("authorization"), Some(&"[REDACTED]".to_string()));
        assert_eq!(redacted.get("x-api-key"), Some(&"[REDACTED]".to_string()));
        assert_eq!(redacted.get("content-type"), Some(&"application/json".to_string()));
        assert_eq!(redacted.get("user-agent"), Some(&"Claude-Code/1.0".to_string()));
    }

    #[tokio::test]
    async fn test_write_entry_to_disk() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let storage_dir = temp_dir.path().to_string_lossy().to_string();
        
        let mut config = crate::config::RequestJournalConfig::default();
        config.enabled = true;
        config.storage_dir = storage_dir.clone();
        
        let writer = RequestJournalWriter::new(config);
        
        let entry = RequestJournalEntry {
            id: "req_test_123".to_string(),
            timestamp: "2026-03-26T10:30:00Z".to_string(),
            client_name: "claude".to_string(),
            user_agent: "Claude-Code/1.0".to_string(),
            method: "POST".to_string(),
            path: "/v1/messages".to_string(),
            provider: "test-provider".to_string(),
            upstream_url: "https://api.example.com/v1/messages".to_string(),
            model: "claude-3-opus".to_string(),
            streaming: true,
            status: 200,
            request_headers: std::collections::HashMap::new(),
            request_content_type: "application/json".to_string(),
            request_body_text: Some(r#"{"model":"claude-3-opus","messages":[]}"#.to_string()),
            request_body_base64: None,
            request_bytes: 100,
            response_bytes: 200,
            failover_chain: None,
            error: None,
            timing: None,
        };

        writer.write_entry(entry.clone()).await;
        
        let expected_path = PathBuf::from(&storage_dir)
            .join("2026-03-26")
            .join("req_test_123.json");
        
        assert!(expected_path.exists(), "Journal file should exist at {:?}", expected_path);
        
        let content = tokio::fs::read_to_string(&expected_path)
            .await
            .expect("Failed to read journal file");
        
        let parsed: RequestJournalEntry = serde_json::from_str(&content)
            .expect("Failed to parse journal file");
        
        assert_eq!(parsed.id, "req_test_123");
        assert_eq!(parsed.client_name, "claude");
        assert_eq!(parsed.model, "claude-3-opus");
    }

    #[tokio::test]
    async fn test_write_entry_disabled() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let storage_dir = temp_dir.path().to_string_lossy().to_string();
        
        let mut config = crate::config::RequestJournalConfig::default();
        config.enabled = false;
        config.storage_dir = storage_dir.clone();
        
        let writer = RequestJournalWriter::new(config);
        
        let entry = RequestJournalEntry {
            id: "req_disabled".to_string(),
            timestamp: "2026-03-26T10:30:00Z".to_string(),
            client_name: "test".to_string(),
            user_agent: "test".to_string(),
            method: "POST".to_string(),
            path: "/test".to_string(),
            provider: "test".to_string(),
            upstream_url: "https://test.com".to_string(),
            model: "test".to_string(),
            streaming: false,
            status: 200,
            request_headers: std::collections::HashMap::new(),
            request_content_type: "application/json".to_string(),
            request_body_text: None,
            request_body_base64: None,
            request_bytes: 0,
            response_bytes: 0,
            failover_chain: None,
            error: None,
            timing: None,
        };

        writer.write_entry(entry).await;
        
        let expected_path = PathBuf::from(&storage_dir)
            .join("2026-03-26")
            .join("req_disabled.json");
        
        assert!(!expected_path.exists(), "Journal file should NOT exist when disabled");
    }
}

/// Format a timing summary as human-readable text.
/// Pure `format!()` string concatenation — no LLM calls, no template engine.
pub fn format_timing_summary(entry: &RequestJournalEntry) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Request {} ({} {}):", entry.id, entry.method, entry.path));
    lines.push(format!("- Model: {}", entry.model));
    lines.push(format!("- Provider: {} ({})", entry.provider, if entry.streaming { "SSE" } else { "non-SSE" }));
    lines.push(format!("- Status: {}", entry.status));

    if let Some(ref timing) = entry.timing {
        lines.push("- Timing breakdown:".to_string());
        lines.push(format!("  · Parse model: {}ms (available providers: {})", timing.parse_model_ms, timing.available_providers));
        lines.push(format!("  · Select provider: {}ms (decision: {}, retries: {})", timing.select_provider_ms, timing.selection_reason, timing.retry_count));
        lines.push(format!("  · Upstream total: {}ms", timing.upstream_total_ms));
        lines.push(format!("  · Upstream TTFB: {}ms", timing.upstream_ttfb_ms));
        if !timing.retry_providers.is_empty() {
            lines.push("  · Attempt breakdown:".to_string());
            for (i, (prov, dur)) in timing.retry_providers.iter().zip(timing.retry_durations_ms.iter()).enumerate() {
                let tag = if i < timing.retry_count as usize { "failed" } else { "success" };
                lines.push(format!("    {}. {}: {}ms ({})", i + 1, prov, dur, tag));
            }
        }
        lines.push(format!("- Data complete: {}", if timing.completed { "yes" } else { "no (partial)" }));
    } else {
        lines.push("- Timing: not collected".to_string());
    }

    if let Some(ref error) = entry.error {
        lines.push(format!("- Error: {}", error));
    }

    lines.join("\n")
}
