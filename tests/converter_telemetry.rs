//! Tests for conversion telemetry

use zz::converter::telemetry::{compute_signature, InMemoryTelemetry, TelemetryConfig, TelemetryContext, NoopTelemetry, Phase, EventKind};
use zz::converter::ApiType;
use bytes::Bytes;

#[test]
fn test_compute_signature() {
    let sig1 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "v1.0.0");
    let sig2 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "v1.0.0");
    let sig3 = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("other"), "v1.0.0");

    assert_eq!(sig1, sig2);
    assert_ne!(sig1, sig3);
    assert_eq!(sig1.len(), 40); // SHA1 produces 40 hex chars
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

#[test]
fn test_noop_telemetry_does_nothing() {
    let ctx = NoopTelemetry;
    // These should not panic
    ctx.report_field_mapped("test", "test");
    ctx.report_field_skipped("test", "test");
    ctx.report_unknown_field("test");
    ctx.report_error(&zz::converter::ConversionError::new(
        zz::converter::ConversionErrorKind::Internal,
        "test",
        "test"
    ));
}

#[tokio::test]
async fn test_in_memory_telemetry_records_events() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    let telemetry = InMemoryTelemetry::new(config, "test-v1.0.0");
    
    let event = zz::converter::telemetry::ConversionEvent {
        ts: chrono::Utc::now(),
        req_id: "test-123".to_string(),
        route: "/a2o/v1/messages".to_string(),
        source: ApiType::Anthropic,
        target: ApiType::OpenAIChat,
        phase: Phase::Request,
        kind: EventKind::Success,
        error_code: None,
        field_path: None,
        upstream_status: None,
        converter_version: "test-v1.0.0",
        sample_id: None,
    };
    
    telemetry.record_event(event).await;
    
    let events = telemetry.get_events(None, None, 10).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].req_id, "test-123");
}

#[tokio::test]
async fn test_in_memory_telemetry_records_samples() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    let telemetry = InMemoryTelemetry::new(config, "test-v1.0.0");
    
    let signature = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "test-v1.0.0");
    let request_body = Bytes::from("{\"test\": \"data\"}");
    let response_body = Some(Bytes::from("{\"result\": \"ok\"}"));
    
    let sample_id = telemetry.record_sample(signature.clone(), request_body.clone(), response_body.clone());
    
    assert!(sample_id > 0);
    
    let sample = telemetry.get_samples_by_signature(&signature);
    assert!(sample.is_some());
    let sample = sample.unwrap();
    assert_eq!(sample.hit_count, 1);
}

#[tokio::test]
async fn test_in_memory_telemetry_dedup_samples() {
    let config = TelemetryConfig {
        enabled: true,
        sample_resave_every: 100, // Use non-zero value
        ..Default::default()
    };
    let telemetry = InMemoryTelemetry::new(config, "test-v1.0.0");
    
    let signature = compute_signature("Anthropic→OpenAIChat", Some("unknown_field"), Some("metadata"), "test-v1.0.0");
    let request_body = Bytes::from("{\"test\": \"data\"}");
    
    let id1 = telemetry.record_sample(signature.clone(), request_body.clone(), None);
    let id2 = telemetry.record_sample(signature.clone(), request_body.clone(), None);
    
    // Should return the same sample ID (dedup)
    assert_eq!(id1, id2);
    
    let sample = telemetry.get_samples_by_signature(&signature).unwrap();
    assert_eq!(sample.hit_count, 2);
}
