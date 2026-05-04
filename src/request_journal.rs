//! Request Journal - Persistent storage for LLM request forensics.
//!
//! Captures complete request details (headers, body) for debugging
//! parameter compatibility and upstream behavior issues.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;

/// Parse raw SSE bytes and assemble extracted content into a readable string.
///
/// Supports both OpenAI and Anthropic streaming formats:
/// - OpenAI: extracts `choices[0].delta.content`
/// - Anthropic: extracts `delta.text` from `content_block_delta` events
///
/// Returns the assembled text if any content was extracted, or None if nothing
/// could be parsed (caller should fall back to raw text).
pub fn assemble_sse_content(raw: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        // Skip empty lines and [DONE] sentinel
        if line.is_empty() || line == "data: [DONE]" {
            continue;
        }
        let json_str = line.strip_prefix("data: ").or_else(|| line.strip_prefix("data:"))?;
        if json_str.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
            // OpenAI format: choices[0].delta.content
            if let Some(content) = val.get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                if !content.is_empty() {
                    parts.push(content.to_string());
                }
                continue;
            }

            // Anthropic format: type == "content_block_delta" → delta.text
            if val.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
                if let Some(text) = val.get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                }
                continue;
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

/// Extract the assistant's reply text from a non-SSE JSON response body.
///
/// Supports:
/// - OpenAI format: `choices[0].message.content`
/// - Anthropic format: `content` array → first `text` block's `text` field
///
/// Returns the extracted text, or None if the JSON couldn't be parsed or
/// the content fields weren't found.
pub fn extract_response_content(raw: &str) -> Option<String> {
    let val = serde_json::from_str::<serde_json::Value>(raw).ok()?;

    // OpenAI format: choices[0].message.content
    if let Some(content) = val.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }

    // Anthropic format: content[0].text (for text blocks)
    if let Some(content_arr) = val.get("content").and_then(|c| c.as_array()) {
        let texts: Vec<String> = content_arr.iter()
            .filter_map(|block| {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }

    None
}

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
    /// Error classification for each attempt — same length as retry_providers
    /// Values: "ok" for success, "timeout"/"connection"/"429"/"quota"/"5xx"/"http_4xx"/"error" for failures
    #[serde(default)]
    pub retry_errors: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body_base64: Option<String>,
    pub response_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failover_chain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Truncated upstream error response body (first 4KB) for non-2xx responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_error_body: Option<String>,
    /// Truncated upstream response body (first 4KB UTF-8) for non-SSE responses
    /// (both success and 4xx pass-through). SSE responses set this to None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_response_body: Option<String>,
    /// Raw SSE event stream (first 4KB) for debugging streaming protocol issues.
    /// Only set for SSE responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse_raw_body: Option<String>,
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

/// Lightweight index entry for fast querying without full deserialization.
#[derive(Debug, Clone)]
struct IndexEntry {
    filename: String,
    timestamp: String,
    status: u16,
    model: String,
    provider: String,
    client_name: String,
    path: String,
    streaming: bool,
    upstream_total_ms: u64,
    timing_completed: bool,
    request_bytes: u64,
    response_bytes: u64,
    upstream_ttfb_ms: u64,
    user_agent: String,
    method: String,
}

impl IndexEntry {
    fn to_summary(&self) -> RequestJournalSummary {
        RequestJournalSummary {
            id: self.filename.clone(),
            timestamp: self.timestamp.clone(),
            client_name: self.client_name.clone(),
            user_agent: self.user_agent.clone(),
            method: self.method.clone(),
            path: self.path.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            streaming: self.streaming,
            status: self.status,
            request_bytes: self.request_bytes,
            response_bytes: self.response_bytes,
            timing_summary: if self.upstream_total_ms > 0 || self.upstream_ttfb_ms > 0 {
                Some(TimingSummary {
                    upstream_total_ms: self.upstream_total_ms,
                    upstream_ttfb_ms: self.upstream_ttfb_ms,
                    completed: self.timing_completed,
                })
            } else {
                None
            },
        }
    }
}

/// Partial JSON structure for extracting index fields without full deserialization.
#[derive(Debug, Clone, serde::Deserialize)]
struct PartialEntry {
    id: String,
    timestamp: String,
    client_name: String,
    user_agent: String,
    method: String,
    path: String,
    provider: String,
    model: String,
    streaming: bool,
    status: u16,
    request_bytes: u64,
    response_bytes: u64,
    timing: Option<PartialTiming>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PartialTiming {
    upstream_total_ms: u64,
    upstream_ttfb_ms: u64,
    completed: bool,
}

impl From<PartialEntry> for IndexEntry {
    fn from(e: PartialEntry) -> Self {
        Self {
            filename: e.id,
            timestamp: e.timestamp,
            status: e.status,
            model: e.model,
            provider: e.provider,
            client_name: e.client_name,
            path: e.path,
            streaming: e.streaming,
            upstream_total_ms: e.timing.as_ref().map_or(0, |t| t.upstream_total_ms),
            timing_completed: e.timing.as_ref().map_or(false, |t| t.completed),
            request_bytes: e.request_bytes,
            response_bytes: e.response_bytes,
            upstream_ttfb_ms: e.timing.as_ref().map_or(0, |t| t.upstream_ttfb_ms),
            user_agent: e.user_agent,
            method: e.method,
        }
    }
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
    /// Lightweight index: date -> list of index entries for that date.
    /// Lazily built on first query, incrementally updated on write.
    file_index: Arc<AsyncRwLock<Option<std::collections::HashMap<String, Vec<IndexEntry>>>>>,
}

impl RequestJournalWriter {
    pub fn new(config: crate::config::RequestJournalConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            created_dirs: Arc::new(AsyncRwLock::new(std::collections::HashSet::new())),
            file_index: Arc::new(AsyncRwLock::new(None)),
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

        // Build index entry before writing (avoids re-reading the file)
        let index_entry = IndexEntry {
            filename: entry.id.clone(),
            timestamp: entry.timestamp.clone(),
            status: entry.status,
            model: entry.model.clone(),
            provider: entry.provider.clone(),
            client_name: entry.client_name.clone(),
            path: entry.path.clone(),
            streaming: entry.streaming,
            upstream_total_ms: entry.timing.as_ref().map_or(0, |t| t.upstream_total_ms),
            timing_completed: entry.timing.as_ref().map_or(false, |t| t.completed),
            request_bytes: entry.request_bytes,
            response_bytes: entry.response_bytes,
            upstream_ttfb_ms: entry.timing.as_ref().map_or(0, |t| t.upstream_ttfb_ms),
            user_agent: entry.user_agent.clone(),
            method: entry.method.clone(),
        };

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
                    // Update index if it has been built
                    let mut index = self.file_index.write().await;
                    if let Some(ref mut idx) = *index {
                        idx.entry(date.to_string())
                            .or_default()
                            .push(index_entry);
                    }

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

    /// Return total entry count from the in-memory index (O(1) after first build).
    /// Returns None if the index has not been built yet.
    pub async fn total_count(&self, storage_dir: &str) -> Option<usize> {
        self.ensure_index(storage_dir).await;
        let index = self.file_index.read().await;
        index.as_ref().map(|idx| idx.values().map(|v| v.len()).sum())
    }

    /// Return unique client names, provider names, and model names from the index.
    pub async fn facets(
        &self,
        storage_dir: &str,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        self.ensure_index(storage_dir).await;
        let index = self.file_index.read().await;
        let idx = match index.as_ref() {
            Some(idx) => idx,
            None => return (Vec::new(), Vec::new(), Vec::new()),
        };

        let mut clients = std::collections::BTreeSet::new();
        let mut providers = std::collections::BTreeSet::new();
        let mut models = std::collections::BTreeSet::new();

        for entries in idx.values() {
            for entry in entries {
                clients.insert(entry.client_name.clone());
                if !entry.provider.is_empty() {
                    providers.insert(entry.provider.clone());
                }
                if !entry.model.is_empty() {
                    models.insert(entry.model.clone());
                }
            }
        }

        (
            clients.into_iter().collect(),
            providers.into_iter().collect(),
            models.into_iter().collect(),
        )
    }

    /// Ensure the file index is built. Lazily scans the storage directory on first call.
    async fn ensure_index(&self, storage_dir: &str) {
        {
            let index = self.file_index.read().await;
            if index.is_some() {
                return;
            }
        }

        let mut index = self.file_index.write().await;
        // Double-check after acquiring write lock
        if index.is_some() {
            return;
        }

        let base_path = PathBuf::from(storage_dir);
        let mut built: std::collections::HashMap<String, Vec<IndexEntry>> =
            std::collections::HashMap::new();

        if !base_path.exists() {
            *index = Some(built);
            return;
        }

        let mut date_dirs = match tokio::fs::read_dir(&base_path).await {
            Ok(dirs) => dirs,
            Err(_) => {
                *index = Some(built);
                return;
            }
        };

        while let Ok(Some(dir_entry)) = date_dirs.next_entry().await {
            if !dir_entry.path().is_dir() {
                continue;
            }
            let date_name = match dir_entry.file_name().to_str() {
                Some(n) if n.len() == 10 && n.contains('-') => n.to_string(),
                _ => continue,
            };

            let mut files = match tokio::fs::read_dir(dir_entry.path()).await {
                Ok(f) => f,
                Err(_) => continue,
            };

            let mut entries = Vec::new();
            while let Ok(Some(file)) = files.next_entry().await {
                let path = file.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    match tokio::fs::read_to_string(&path).await {
                        Ok(content) => {
                            match serde_json::from_str::<PartialEntry>(&content) {
                                Ok(partial) => entries.push(IndexEntry::from(partial)),
                                Err(e) => {
                                    tracing::warn!(
                                        path = ?path,
                                        error = %e,
                                        "Failed to parse journal entry for index"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                path = ?path,
                                error = %e,
                                "Failed to read journal entry for index"
                            );
                        }
                    }
                }
            }

            // Sort entries within each date by timestamp descending
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            built.insert(date_name, entries);
        }

        tracing::info!(
            dates = built.len(),
            total = built.values().map(|v| v.len()).sum::<usize>(),
            "Request journal index built"
        );
        *index = Some(built);
    }

    /// Remove expired date directories from the index (called after cleanup).
    pub async fn invalidate_index_dates(&self, dates: &[String]) {
        let mut index = self.file_index.write().await;
        if let Some(ref mut idx) = *index {
            for date in dates {
                idx.remove(date);
            }
        }
    }

    /// Update an existing journal entry's timing and response_bytes in-place.
    /// Used by SSE streams to finalize timing after streaming completes.
    pub async fn finalize_sse_entry(
        &self,
        entry_id: &str,
        timestamp: &str,
        response_bytes: u64,
        upstream_total_ms: u64,
        captured_body: Vec<u8>,
    ) {
        let storage_dir = self.storage_dir();

        let date = if timestamp.len() >= 10 {
            &timestamp[..10]
        } else {
            return;
        };

        let file_path = PathBuf::from(&storage_dir)
            .join(date)
            .join(format!("{}.json", entry_id));

        if !file_path.exists() {
            return;
        }

        // Read existing entry
        let content = match tokio::fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    id = %entry_id,
                    error = %e,
                    "Failed to read journal entry for SSE finalization"
                );
                return;
            }
        };

        // Parse, update timing, and write back
        let mut entry: RequestJournalEntry = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    id = %entry_id,
                    error = %e,
                    "Failed to parse journal entry for SSE finalization"
                );
                return;
            }
        };

        entry.response_bytes = response_bytes;

        // Store captured SSE body: assemble readable content and keep raw for debugging.
        if !captured_body.is_empty() {
            let raw_text = String::from_utf8_lossy(&captured_body).into_owned();
            if let Some(assembled) = assemble_sse_content(&raw_text) {
                entry.response_body_text = Some(assembled);
                entry.sse_raw_body = Some(raw_text);
            } else {
                // Could not parse SSE format — store raw as-is
                entry.response_body_text = Some(raw_text);
            }
        }

        if let Some(ref mut timing) = entry.timing {
            timing.upstream_total_ms = upstream_total_ms;
            timing.completed = true;
        }

        match serde_json::to_string_pretty(&entry) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&file_path, json).await {
                    tracing::warn!(
                        id = %entry_id,
                        error = %e,
                        "Failed to write finalized SSE journal entry"
                    );
                } else {
                    tracing::debug!(
                        id = %entry_id,
                        response_bytes = response_bytes,
                        upstream_total_ms = upstream_total_ms,
                        "Finalized SSE journal entry"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    id = %entry_id,
                    error = %e,
                    "Failed to serialize finalized SSE journal entry"
                );
            }
        }

        // Update index if present
        let mut index = self.file_index.write().await;
        if let Some(ref mut idx) = *index {
            if let Some(entries) = idx.get_mut(date) {
                for entry in entries.iter_mut() {
                    if entry.filename == entry_id {
                        entry.response_bytes = response_bytes;
                        entry.upstream_total_ms = upstream_total_ms;
                        entry.timing_completed = true;
                        break;
                    }
                }
            }
        }
    }
}

pub async fn list_entries(
    storage_dir: &str,
    query: JournalQuery,
    offset: usize,
    limit: usize,
    writer: Option<&RequestJournalWriter>,
) -> Result<(Vec<RequestJournalSummary>, usize), String> {
    // If we have a writer reference, use the index
    if let Some(w) = writer {
        return list_entries_indexed(w, storage_dir, query, offset, limit).await;
    }

    // Fallback: direct scan (for export or backward compat)
    list_entries_scan(storage_dir, query, offset, limit).await
}

/// List entries using the in-memory index — avoids full deserialization.
async fn list_entries_indexed(
    writer: &RequestJournalWriter,
    storage_dir: &str,
    query: JournalQuery,
    offset: usize,
    limit: usize,
) -> Result<(Vec<RequestJournalSummary>, usize), String> {
    writer.ensure_index(storage_dir).await;

    let index = writer.file_index.read().await;
    let idx = match index.as_ref() {
        Some(idx) => idx,
        None => return Ok((Vec::new(), 0)),
    };

    // Determine which dates to scan
    let date_keys: Vec<String> = if let Some(ref date) = query.date {
        if date.contains("..") || date.contains('/') || date.contains('\\') {
            return Err("Invalid date filter".to_string());
        }
        vec![date.clone()]
    } else {
        let mut keys: Vec<String> = idx.keys().cloned().collect();
        keys.sort();
        keys.reverse();
        keys
    };

    // Collect matching entries from index (pure memory operation)
    let mut matched: Vec<&IndexEntry> = Vec::new();

    for date_key in &date_keys {
        if let Some(entries) = idx.get(date_key) {
            for entry in entries {
                if matches_query_index(entry, &query) {
                    matched.push(entry);
                }
            }
        }
    }

    // Sort by timestamp descending (entries within each date are already sorted,
    // but we need cross-date merge)
    matched.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total = matched.len();
    let summaries: Vec<RequestJournalSummary> = matched
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|e| e.to_summary())
        .collect();

    Ok((summaries, total))
}

/// Check if an index entry matches the query filters.
fn matches_query_index(entry: &IndexEntry, query: &JournalQuery) -> bool {
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
        if entry.timing_completed && entry.status < 400 {
            return false;
        }
    }

    if query.slow_only {
        if entry.upstream_total_ms <= 3000 {
            return false;
        }
    }

    true
}

/// Direct scan fallback — used when no writer reference is available (e.g. export).
async fn list_entries_scan(
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
    limit: usize,
) -> Result<Vec<RequestJournalEntry>, String> {
    list_entries_full(storage_dir, query, limit).await
}

/// List full entries (no pagination — used for export).
/// Caps results at `limit` to bound memory usage.
async fn list_entries_full(
    storage_dir: &str,
    query: JournalQuery,
    limit: usize,
) -> Result<Vec<RequestJournalEntry>, String> {
    let base_path = PathBuf::from(storage_dir);

    if !base_path.exists() {
        return Ok(Vec::new());
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
                                    if entries.len() >= limit {
                                        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                                        return Ok(entries);
                                    }
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
    Ok(entries)
}

/// Delete date directories older than `retention_days` from the journal storage.
/// Returns the list of deleted date directory names.
pub async fn cleanup_expired(storage_dir: &str, retention_days: u64) -> Vec<String> {
    if retention_days == 0 || !std::path::Path::new(storage_dir).exists() {
        return Vec::new();
    }

    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

    let mut deleted = Vec::new();
    let mut date_dirs = match tokio::fs::read_dir(storage_dir).await {
        Ok(dirs) => dirs,
        Err(_) => return deleted,
    };

    while let Ok(Some(entry)) = date_dirs.next_entry().await {
        if !entry.path().is_dir() {
            continue;
        }
        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        // Only consider date-like directories (YYYY-MM-DD)
        if name.len() != 10 || !name.contains('-') {
            continue;
        }
        if name.as_str() < cutoff_str.as_str() {
            match tokio::fs::remove_dir_all(entry.path()).await {
                Ok(_) => {
                    tracing::info!(
                        dir = %name,
                        "Cleaned up expired request journal directory"
                    );
                    deleted.push(name);
                }
                Err(e) => {
                    tracing::warn!(
                        dir = %name,
                        error = %e,
                        "Failed to remove expired request journal directory"
                    );
                }
            }
        }
    }

    deleted
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
            response_body_text: None,
            response_body_base64: None,
            response_bytes: 200,
            failover_chain: None,
            error: None,
            upstream_error_body: None,
            upstream_response_body: None,
            sse_raw_body: None,
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
            response_body_text: None,
            response_body_base64: None,
            response_bytes: 0,
            failover_chain: None,
            error: None,
            upstream_error_body: None,
            upstream_response_body: None,
            sse_raw_body: None,
            timing: None,
        };

        writer.write_entry(entry).await;

        let expected_path = PathBuf::from(&storage_dir)
            .join("2026-03-26")
            .join("req_disabled.json");

        assert!(!expected_path.exists(), "Journal file should NOT exist when disabled");
    }

    #[tokio::test]
    async fn test_list_entries_indexed() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let storage_dir = temp_dir.path().to_string_lossy().to_string();

        let mut config = crate::config::RequestJournalConfig::default();
        config.enabled = true;
        config.storage_dir = storage_dir.clone();

        let writer = RequestJournalWriter::new(config);

        // Write multiple entries
        for i in 0..5 {
            let entry = RequestJournalEntry {
                id: format!("req_idx_{}", i),
                timestamp: format!("2026-03-26T10:3{}:00Z", i),
                client_name: if i % 2 == 0 { "claude" } else { "cursor" }.to_string(),
                user_agent: "test".to_string(),
                method: "POST".to_string(),
                path: "/v1/messages".to_string(),
                provider: "test-provider".to_string(),
                upstream_url: "https://api.example.com/v1/messages".to_string(),
                model: "claude-3-opus".to_string(),
                streaming: true,
                status: if i == 3 { 500 } else { 200 },
                request_headers: std::collections::HashMap::new(),
                request_content_type: "application/json".to_string(),
                request_body_text: None,
                request_body_base64: None,
                request_bytes: 100,
                response_body_text: None,
                response_body_base64: None,
                response_bytes: 200,
                failover_chain: None,
                error: None,
                upstream_error_body: None,
                upstream_response_body: None,
                sse_raw_body: None,
                timing: None,
            };
            writer.write_entry(entry).await;
        }

        // Query all
        let (entries, total) = list_entries(
            &storage_dir,
            JournalQuery::default(),
            0,
            10,
            Some(&writer),
        )
        .await
        .unwrap();

        assert_eq!(total, 5);
        assert_eq!(entries.len(), 5);
        // Should be sorted by timestamp descending
        assert_eq!(entries[0].id, "req_idx_4");

        // Query with client filter
        let (entries, total) = list_entries(
            &storage_dir,
            JournalQuery {
                client: Some("claude".to_string()),
                ..Default::default()
            },
            0,
            10,
            Some(&writer),
        )
        .await
        .unwrap();

        assert_eq!(total, 3); // indices 0, 2, 4
        assert!(entries.iter().all(|e| e.client_name == "claude"));

        // Query with status filter
        let (entries, total) = list_entries(
            &storage_dir,
            JournalQuery {
                status: Some(500),
                ..Default::default()
            },
            0,
            10,
            Some(&writer),
        )
        .await
        .unwrap();

        assert_eq!(total, 1);
        assert_eq!(entries[0].id, "req_idx_3");

        // Pagination
        let (entries, total) = list_entries(
            &storage_dir,
            JournalQuery::default(),
            2,
            2,
            Some(&writer),
        )
        .await
        .unwrap();

        assert_eq!(total, 5);
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let storage_dir = temp_dir.path().to_string_lossy().to_string();

        // Create some date directories
        let old_date = "2020-01-01";
        let recent_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

        tokio::fs::create_dir_all(std::path::Path::new(&storage_dir).join(old_date))
            .await
            .unwrap();
        tokio::fs::create_dir_all(std::path::Path::new(&storage_dir).join(&recent_date))
            .await
            .unwrap();

        // Write a dummy file in each
        tokio::fs::write(
            std::path::Path::new(&storage_dir).join(old_date).join("test.json"),
            "{}",
        )
        .await
        .unwrap();
        tokio::fs::write(
            std::path::Path::new(&storage_dir)
                .join(&recent_date)
                .join("test.json"),
            "{}",
        )
        .await
        .unwrap();

        let deleted = cleanup_expired(&storage_dir, 7).await;
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], old_date);

        // Old dir should be gone, recent should remain
        assert!(
            !std::path::Path::new(&storage_dir)
                .join(old_date)
                .exists()
        );
        assert!(
            std::path::Path::new(&storage_dir)
                .join(&recent_date)
                .exists()
        );
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
                let err_label = timing.retry_errors.get(i)
                    .filter(|e| *e != "ok")
                    .map(|e| format!(" ({})", e))
                    .unwrap_or_default();
                lines.push(format!("    {}. {}: {}ms ({}){}", i + 1, prov, dur, tag, err_label));
            }
        }
        lines.push(format!("- Data complete: {}", if timing.completed { "yes" } else { "no (partial)" }));
    } else {
        lines.push("- Timing: not collected".to_string());
    }

    if let Some(ref error) = entry.error {
        lines.push(format!("- Error: {}", error));
    }

    if let Some(ref body) = entry.upstream_error_body {
        let preview = if body.len() > 500 { format!("{}...(truncated)", &body[..500]) } else { body.clone() };
        lines.push(format!("- Upstream error body: {}", preview));
    }

    lines.join("\n")
}
