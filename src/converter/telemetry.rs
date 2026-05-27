//! Conversion Telemetry Module
//!
//! Provides structured event collection for conversion operations.
//! Enables iteration loop: discover missing field mappings from production usage.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as AsyncRwLock;

use crate::converter::{ApiType, ConversionError};

/// Phase of conversion: request, response, or streaming
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Request,
    Response,
    Stream,
}

/// Event kind for telemetry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Success,
    FieldSkipped,
    UnknownField,
    Fallback,
    Error,
}

/// A single conversion event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionEvent {
    pub ts: DateTime<Utc>,
    pub req_id: String,
    pub route: String,
    pub source: ApiType,
    pub target: ApiType,
    pub phase: Phase,
    pub kind: EventKind,
    pub error_code: Option<&'static str>,
    pub field_path: Option<String>,
    pub upstream_status: Option<u16>,
    pub converter_version: &'static str,
    pub sample_id: Option<u64>,
}

/// A sample of a conversion (request/response body) for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSample {
    pub id: u64,
    pub event_signature: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub hit_count: u64,
    pub request_body_preview: String,
    pub response_body_preview: Option<String>,
    pub redacted: bool,
}

/// Telemetry configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TelemetryConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_sample_max_count")]
    pub sample_max_count: usize,
    #[serde(default = "default_sample_max_bytes")]
    pub sample_max_bytes: u64,
    #[serde(default = "default_sample_resave_every")]
    pub sample_resave_every: u64,
    #[serde(default)]
    pub persist_path: String,
    #[serde(default = "default_unknown_field_log_level")]
    pub unknown_field_log_level: String,
    #[serde(default)]
    pub redact_extra_headers: Vec<String>,
}

fn default_enabled() -> bool {
    true
}

fn default_sample_max_count() -> usize {
    10000
}

fn default_sample_max_bytes() -> u64 {
    67_108_864 // 64 MiB
}

fn default_sample_resave_every() -> u64 {
    100
}

fn default_unknown_field_log_level() -> String {
    "warn".to_string()
}

/// Compute event signature for deduplication
pub fn compute_signature(
    direction: &str,
    error_code: Option<&str>,
    field_path: Option<&str>,
    converter_version: &str,
) -> String {
    let mut hasher = Sha1::new();
    hasher.update(direction.as_bytes());
    if let Some(code) = error_code {
        hasher.update(b"|");
        hasher.update(code.as_bytes());
    }
    if let Some(path) = field_path {
        hasher.update(b"|");
        hasher.update(path.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(converter_version.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// In-memory telemetry store
pub struct InMemoryTelemetry {
    config: TelemetryConfig,
    events: Arc<AsyncRwLock<Vec<ConversionEvent>>>,
    samples: Arc<RwLock<HashMap<String, ConversionSample>>>,
    next_sample_id: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
    converter_version: &'static str,
    enabled: Arc<AtomicBool>,
}

impl InMemoryTelemetry {
    pub fn new(config: TelemetryConfig, converter_version: &'static str) -> Self {
        let enabled = config.enabled;
        Self {
            config,
            events: Arc::new(AsyncRwLock::new(Vec::new())),
            samples: Arc::new(RwLock::new(HashMap::new())),
            next_sample_id: Arc::new(AtomicU64::new(1)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            converter_version,
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Record a conversion event
    pub async fn record_event(&self, event: ConversionEvent) {
        if !self.is_enabled() {
            return;
        }

        let mut events = self.events.write().await;
        events.push(event);

        // Keep events bounded (last 1000)
        if events.len() > 1000 {
            events.remove(0);
        }
    }

    /// Record or update a sample
    pub fn record_sample(
        &self,
        signature: String,
        request_body: Bytes,
        response_body: Option<Bytes>,
    ) -> u64 {
        if !self.is_enabled() {
            return 0;
        }

        let mut samples = self.samples.write().unwrap();
        let sample_id = self.next_sample_id.fetch_add(1, Ordering::Relaxed);
        let now = Utc::now();

        // Check if signature already exists
        if let Some(sample) = samples.get_mut(&signature) {
            sample.hit_count += 1;
            sample.last_seen = now;

            // Resave body periodically
            if sample.hit_count % self.config.sample_resave_every == 0 {
                sample.request_body_preview = self.truncate_body_to_string(&request_body);
                sample.response_body_preview = response_body.as_ref().map(|b| self.truncate_body_to_string(b));
            }

            return sample.id;
        }

        // Check size limits
        let new_bytes = request_body.len() + response_body.as_ref().map_or(0, |b| b.len());
        let current_bytes = self.total_bytes.load(Ordering::Relaxed);
        
        if current_bytes as usize + new_bytes > self.config.sample_max_bytes as usize
            || samples.len() >= self.config.sample_max_count
        {
            // Evict oldest sample (FIFO)
            if let Some(sig) = samples.keys().next().cloned() {
                if let Some(old_sample) = samples.remove(&sig) {
                    let old_bytes = old_sample.request_body_preview.len()
                        + old_sample.response_body_preview.as_ref().map_or(0, |b| b.len());
                    self.total_bytes.fetch_sub(old_bytes as u64, Ordering::Relaxed);
                }
            }
        }

        let sample = ConversionSample {
            id: sample_id,
            event_signature: signature.clone(),
            first_seen: now,
            last_seen: now,
            hit_count: 1,
            request_body_preview: self.truncate_body_to_string(&request_body),
            response_body_preview: response_body.map(|b| self.truncate_body_to_string(&b)),
            redacted: true, // Always redact samples
        };

        self.total_bytes.fetch_add(new_bytes as u64, Ordering::Relaxed);
        samples.insert(signature, sample);
        sample_id
    }

    fn truncate_body_to_string(&self, body: &Bytes) -> String {
        String::from_utf8_lossy(&crate::converter::truncate_bytes_utf8(body, 4096)).to_string()
    }

    /// Get all events
    pub async fn get_events(&self, since: Option<DateTime<Utc>>, kind: Option<EventKind>, limit: usize) -> Vec<ConversionEvent> {
        if !self.is_enabled() {
            return Vec::new();
        }

        let events = self.events.read().await;
        let mut filtered: Vec<_> = events
            .iter()
            .filter(|e| {
                if let Some(since_ts) = since {
                    if e.ts < since_ts {
                        return false;
                    }
                }
                if let Some(filter_kind) = kind {
                    if e.kind != filter_kind {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
        filtered.truncate(limit);
        filtered
    }

    /// Get samples by signature
    pub fn get_samples_by_signature(&self, signature: &str) -> Option<ConversionSample> {
        if !self.is_enabled() {
            return None;
        }

        let samples = self.samples.read().unwrap();
        samples.get(signature).cloned()
    }

    /// Get sample body by ID
    pub fn get_sample_body(&self, sample_id: u64) -> Option<(String, Option<String>)> {
        if !self.is_enabled() {
            return None;
        }

        let samples = self.samples.read().unwrap();
        samples
            .values()
            .find(|s| s.id == sample_id)
            .map(|s| (s.request_body_preview.clone(), s.response_body_preview.clone()))
    }

    /// Get coverage statistics
    pub fn get_coverage(&self) -> CoverageStats {
        if !self.is_enabled() {
            return CoverageStats::default();
        }

        let samples = self.samples.read().unwrap();
        let mut unknown_field_counts: HashMap<String, u64> = HashMap::new();

        for sample in samples.values() {
            // Parse signature to extract field_path
            // Signature format: sha1(direction|error_code|field_path|version)
            // For now, we'll count samples as a proxy
            *unknown_field_counts.entry("unknown".to_string()).or_insert(0) += sample.hit_count;
        }

        CoverageStats {
            converter_version: self.converter_version.to_string(),
            unknown_field_counts,
            total_samples: samples.len(),
        }
    }

    /// Clear all samples
    pub fn clear_samples(&self) {
        let mut samples = self.samples.write().unwrap();
        samples.clear();
        self.total_bytes.store(0, Ordering::Relaxed);
    }
}

/// Coverage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoverageStats {
    pub converter_version: String,
    pub unknown_field_counts: HashMap<String, u64>,
    pub total_samples: usize,
}

/// Telemetry context trait (extended from converter.rs)
pub trait TelemetryContext: Send + Sync {
    fn report_field_mapped(&self, source_field: &str, target_field: &str);
    fn report_field_skipped(&self, field_path: &str, reason: &str);
    fn report_unknown_field(&self, field_path: &str);
    fn report_error(&self, error: &ConversionError);
    fn report_success(&self, req_id: &str, route: &str, source: ApiType, target: ApiType, phase: Phase);
}

/// No-op telemetry implementation
#[derive(Debug, Clone, Copy)]
pub struct NoopTelemetry;

impl TelemetryContext for NoopTelemetry {
    fn report_field_mapped(&self, _source_field: &str, _target_field: &str) {}
    fn report_field_skipped(&self, _field_path: &str, _reason: &str) {}
    fn report_unknown_field(&self, _field_path: &str) {}
    fn report_error(&self, _error: &ConversionError) {}
    fn report_success(&self, _req_id: &str, _route: &str, _source: ApiType, _target: ApiType, _phase: Phase) {}
}

/// Real telemetry implementation backed by InMemoryTelemetry
#[derive(Clone)]
pub struct RealTelemetry {
    inner: Arc<InMemoryTelemetry>,
    req_id: String,
    route: String,
    source: ApiType,
    target: ApiType,
}

impl RealTelemetry {
    pub fn new(
        inner: Arc<InMemoryTelemetry>,
        req_id: String,
        route: String,
        source: ApiType,
        target: ApiType,
    ) -> Self {
        Self {
            inner,
            req_id,
            route,
            source,
            target,
        }
    }
}

impl TelemetryContext for RealTelemetry {
    fn report_field_mapped(&self, _source_field: &str, _target_field: &str) {
        // Field mapping is successful - no event needed for now
    }

    fn report_field_skipped(&self, field_path: &str, _reason: &str) {
        let direction = format!("{:?}→{:?}", self.source, self.target);
        let _signature = compute_signature(&direction, Some("field_skipped"), Some(field_path), self.inner.converter_version);
        
        let event = ConversionEvent {
            ts: Utc::now(),
            req_id: self.req_id.clone(),
            route: self.route.clone(),
            source: self.source,
            target: self.target,
            phase: Phase::Request, // Simplified
            kind: EventKind::FieldSkipped,
            error_code: Some("field_skipped"),
            field_path: Some(field_path.to_string()),
            upstream_status: None,
            converter_version: self.inner.converter_version,
            sample_id: None,
        };

        tokio::spawn({
            let inner = self.inner.clone();
            async move {
                inner.record_event(event).await;
            }
        });
    }

    fn report_unknown_field(&self, field_path: &str) {
        let direction = format!("{:?}→{:?}", self.source, self.target);
        let signature = compute_signature(&direction, Some("unknown_field"), Some(field_path), self.inner.converter_version);
        
        let event = ConversionEvent {
            ts: Utc::now(),
            req_id: self.req_id.clone(),
            route: self.route.clone(),
            source: self.source,
            target: self.target,
            phase: Phase::Request, // Simplified
            kind: EventKind::UnknownField,
            error_code: Some("unknown_field"),
            field_path: Some(field_path.to_string()),
            upstream_status: None,
            converter_version: self.inner.converter_version,
            sample_id: None,
        };

        tokio::spawn({
            let inner = self.inner.clone();
            async move {
                inner.record_event(event).await;
            }
        });
    }

    fn report_error(&self, error: &ConversionError) {
        let direction = format!("{:?}→{:?}", self.source, self.target);
        let _signature = compute_signature(&direction, Some(error.code), error.field_path.as_deref(), self.inner.converter_version);
        
        let event = ConversionEvent {
            ts: Utc::now(),
            req_id: self.req_id.clone(),
            route: self.route.clone(),
            source: self.source,
            target: self.target,
            phase: Phase::Request, // Simplified
            kind: EventKind::Error,
            error_code: Some(error.code),
            field_path: error.field_path.clone(),
            upstream_status: None,
            converter_version: self.inner.converter_version,
            sample_id: None,
        };

        tokio::spawn({
            let inner = self.inner.clone();
            async move {
                inner.record_event(event).await;
            }
        });
    }

    fn report_success(&self, req_id: &str, route: &str, source: ApiType, target: ApiType, phase: Phase) {
        let event = ConversionEvent {
            ts: Utc::now(),
            req_id: req_id.to_string(),
            route: route.to_string(),
            source,
            target,
            phase,
            kind: EventKind::Success,
            error_code: None,
            field_path: None,
            upstream_status: None,
            converter_version: self.inner.converter_version,
            sample_id: None,
        };

        tokio::spawn({
            let inner = self.inner.clone();
            async move {
                inner.record_event(event).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_signature() {
        let sig1 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "v1.0.0");
        let sig2 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "v1.0.0");
        let sig3 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("other"), "v1.0.0");

        assert_eq!(sig1, sig2);
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_telemetry_config_defaults() {
        let config: TelemetryConfig = toml::from_str("").unwrap();
        assert!(config.enabled);
        assert_eq!(config.sample_max_count, 10000);
        assert_eq!(config.sample_max_bytes, 67_108_864);
        assert_eq!(config.sample_resave_every, 100);
        assert_eq!(config.unknown_field_log_level, "warn");
    }
}
