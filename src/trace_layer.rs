//! Custom tracing Layer for collecting per-request span data.
//!
//! Implements tail-based sampling: spans are always collected in memory,
//! and the sampling decision is made when the root span closes, based on
//! error status, duration, and configured sampling rates.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{Id, Level};
use tracing_subscriber::layer::Context;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A single span within a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub span_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    /// Milliseconds since trace root start
    pub start_ms: u64,
    pub duration_ms: u64,
    pub fields: serde_json::Value,
}

/// A complete trace for one request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    pub trace_id: String,
    pub spans: Vec<SpanRecord>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Internal state for tracking open spans
// ---------------------------------------------------------------------------

struct SpanState {
    name: &'static str,
    target: &'static str,
    level: Level,
    start: Instant,
    fields: HashMap<String, String>,
    parent_id: Option<Id>,
    /// The root span id this span belongs to (resolved on creation)
    root_id: Id,
}

struct RootInfo {
    request_id: String,
    start: Instant,
}

// ---------------------------------------------------------------------------
// Tail-based sampler
// ---------------------------------------------------------------------------

pub struct TraceSampler {
    config: std::sync::RwLock<crate::config::TracingConfig>,
    dropped_count: AtomicU64,
}

impl TraceSampler {
    pub fn new(config: crate::config::TracingConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            dropped_count: AtomicU64::new(0),
        }
    }

    pub fn update_config(&self, new_config: crate::config::TracingConfig) {
        *self.config.write().unwrap() = new_config;
    }

    /// Evaluate sampling at root span close time.
    /// `duration_ms`: total root span duration
    /// `is_error`: whether the request resulted in an error status
    pub fn should_sample(&self, duration_ms: u64, is_error: bool) -> bool {
        let config = self.config.read().unwrap();
        match config.sampling_mode.as_str() {
            "fixed" => rand::random::<f64>() < config.base_rate,
            _ => {
                // adaptive (default)
                if is_error {
                    return rand::random::<f64>() < config.error_sampling;
                }
                if duration_ms > config.slow_threshold_ms {
                    return true;
                }
                rand::random::<f64>() < config.base_rate
            }
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    pub fn inc_dropped(&self) {
        self.dropped_count.fetch_add(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// The Layer
// ---------------------------------------------------------------------------

pub struct JournalTraceLayer {
    /// Active (open) spans keyed by their tracing Id
    active_spans: std::sync::Mutex<HashMap<Id, SpanState>>,
    /// Maps root span Id → accumulated child SpanRecords
    completed_spans: std::sync::Mutex<HashMap<Id, Vec<SpanRecord>>>,
    /// Root span metadata
    root_info: std::sync::Mutex<HashMap<Id, RootInfo>>,
    /// Channel to send completed traces to the writer
    trace_sender: mpsc::Sender<TraceRecord>,
    /// Sampler
    sampler: Arc<TraceSampler>,
}

impl JournalTraceLayer {
    pub fn new_with_sampler(
        sampler: Arc<TraceSampler>,
        trace_sender: mpsc::Sender<TraceRecord>,
    ) -> Self {
        Self {
            active_spans: std::sync::Mutex::new(HashMap::new()),
            completed_spans: std::sync::Mutex::new(HashMap::new()),
            root_info: std::sync::Mutex::new(HashMap::new()),
            trace_sender,
            sampler,
        }
    }

    /// Resolve the root span id for a given span by walking parent chain.
    fn resolve_root_id(&self, parent_id: &Id) -> Option<Id> {
        let spans = self.active_spans.lock().unwrap();
        let mut current = parent_id.clone();
        loop {
            if let Some(state) = spans.get(&current) {
                if state.parent_id.is_none() {
                    // This parent is a root span
                    return Some(current);
                }
                current = state.parent_id.as_ref()?.clone();
            } else {
                return None;
            }
        }
    }

    fn extract_fields(attrs: &tracing::span::Attributes<'_>) -> HashMap<String, String> {
        let mut fields = HashMap::new();
        attrs.record(&mut FieldVisitor(&mut fields));
        fields
    }
}

/// Visitor to extract span fields into a HashMap.
struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_string(), format!("{:?}", value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
}

impl<S> tracing_subscriber::Layer<S> for JournalTraceLayer
where
    S: tracing::Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &Id,
        _ctx: Context<'_, S>,
    ) {
        let fields = Self::extract_fields(attrs);
        let metadata = attrs.metadata();

        // Determine if this is a root span or child span
        let (parent_id, root_id) = if attrs.is_root() {
            // Root span — check for request_id field
            let request_id = fields
                .get("request_id")
                .cloned()
                .unwrap_or_else(|| format!("{:?}", id));

            let root_id = id.clone();
            self.root_info.lock().unwrap().insert(
                id.clone(),
                RootInfo {
                    request_id,
                    start: Instant::now(),
                },
            );
            (None, root_id)
        } else if let Some(parent) = attrs.parent() {
            // Child span — resolve root
            let root_id = self
                .resolve_root_id(&parent)
                .unwrap_or_else(|| parent.clone());
            (Some(parent.clone()), root_id)
        } else {
            // Implicit parent (contextual) — try to find current root
            let root_id = id.clone();
            (None, root_id)
        };

        let state = SpanState {
            name: metadata.name(),
            target: metadata.target(),
            level: *metadata.level(),
            start: Instant::now(),
            fields,
            parent_id,
            root_id,
        };

        self.active_spans.lock().unwrap().insert(id.clone(), state);
    }

    fn on_record(
        &self,
        id: &Id,
        values: &tracing::span::Record<'_>,
        _ctx: Context<'_, S>,
    ) {
        let mut active = self.active_spans.lock().unwrap();
        if let Some(state) = active.get_mut(id) {
            values.record(&mut FieldVisitor(&mut state.fields));
        }
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        // 1. Remove from active_spans and release lock immediately to avoid
        //    holding multiple mutexes simultaneously (deadlock prevention).
        let state = {
            let mut active = self.active_spans.lock().unwrap();
            match active.remove(&id) {
                Some(s) => s,
                None => return,
            }
        };

        let duration_ms = state.start.elapsed().as_millis() as u64;

        // Build span record
        let span_record = SpanRecord {
            span_id: format!("{:?}", id),
            parent_id: state.parent_id.map(|p| format!("{:?}", p)),
            name: state.name.to_string(),
            start_ms: 0, // Will be filled when collecting under root
            duration_ms,
            fields: serde_json::to_value(&state.fields).unwrap_or(serde_json::Value::Null),
        };

        // 2. Acquire root_info lock only after active_spans is released.
        let mut root_info = self.root_info.lock().unwrap();
        let is_root = root_info.contains_key(&id);

        if is_root {
            let root = root_info.remove(&id).unwrap();
            drop(root_info); // Release before acquiring completed_spans.

            let request_id = root.request_id;

            // 3. Acquire completed_spans only after root_info is released.
            let mut completed = self.completed_spans.lock().unwrap();
            let mut spans: Vec<SpanRecord> = completed.remove(&id).unwrap_or_default();
            drop(completed);

            // Add this root span record (relative start = 0)
            let mut root_record = span_record;
            root_record.start_ms = 0;
            spans.insert(0, root_record);

            // Evaluate sampling
            let is_error = spans.iter().any(|s| {
                s.fields
                    .as_object()
                    .map(|o| o.contains_key("error"))
                    .unwrap_or(false)
                    || s.name.contains("error")
            });

            if self.sampler.should_sample(duration_ms, is_error) {
                let trace = TraceRecord {
                    trace_id: request_id,
                    spans,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };

                if self.trace_sender.try_send(trace).is_err() {
                    self.sampler.inc_dropped();
                }
            }
        } else {
            // Child span — accumulate under root
            let mut span_with_time = span_record;
            // Calculate start_ms relative to root
            if let Some(root_info_ref) = root_info.get(&state.root_id) {
                span_with_time.start_ms = root_info_ref
                    .start
                    .elapsed()
                    .saturating_sub(state.start.elapsed())
                    .as_millis() as u64;
            }
            drop(root_info); // Release before acquiring completed_spans.

            let mut completed = self.completed_spans.lock().unwrap();
            completed
                .entry(state.root_id)
                .or_default()
                .push(span_with_time);
        }
    }
}

// ---------------------------------------------------------------------------
// Async writer task
// ---------------------------------------------------------------------------

pub async fn spawn_trace_writer(
    mut receiver: mpsc::Receiver<TraceRecord>,
    storage_dir: String,
) {
    let mut created_dirs: HashMap<String, bool> = HashMap::new();

    while let Some(trace) = receiver.recv().await {
        if let Err(e) = write_trace(&storage_dir, &trace, &mut created_dirs).await {
            tracing::warn!(error = %e, "Failed to write trace file");
        }
    }
}

async fn write_trace(
    storage_dir: &str,
    trace: &TraceRecord,
    created_dirs: &mut HashMap<String, bool>,
) -> Result<(), std::io::Error> {
    // Extract date from created_at (first 10 chars of ISO timestamp)
    let date = trace.created_at.get(..10).unwrap_or("unknown");
    let dir = PathBuf::from(storage_dir).join(date);

    if !created_dirs.contains_key(date) {
        tokio::fs::create_dir_all(&dir).await?;
        created_dirs.insert(date.to_string(), true);
    }

    let path = dir.join(format!("{}.trace.json", trace.trace_id));
    let json = serde_json::to_string_pretty(trace)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    tokio::fs::write(&path, json).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Query function
// ---------------------------------------------------------------------------

/// Delete date directories older than `retention_days` from the trace storage.
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
        if name.len() != 10 || !name.contains('-') {
            continue;
        }
        if name.as_str() < cutoff_str.as_str() {
            match tokio::fs::remove_dir_all(entry.path()).await {
                Ok(_) => {
                    tracing::info!(
                        dir = %name,
                        "Cleaned up expired trace directory"
                    );
                    deleted.push(name);
                }
                Err(e) => {
                    tracing::warn!(
                        dir = %name,
                        error = %e,
                        "Failed to remove expired trace directory"
                    );
                }
            }
        }
    }

    deleted
}

pub async fn get_trace(storage_dir: &str, trace_id: &str) -> Option<TraceRecord> {
    // Reject path traversal attempts in trace_id
    if trace_id.contains("..") || trace_id.contains('/') || trace_id.contains('\\') {
        return None;
    }
    // Scan date directories for the trace file
    let base = PathBuf::from(storage_dir);
    let mut entries = match tokio::fs::read_dir(&base).await {
        Ok(e) => e,
        Err(_) => return None,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path().join(format!("{}.trace.json", trace_id));
        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await.ok()?;
            return serde_json::from_str(&content).ok();
        }
    }

    None
}
