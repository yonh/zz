//! API Converter Module
//!
//! This module provides protocol conversion between different LLM API formats.
//! It implements the converter skeleton for Anthropic ↔ OpenAI Chat Completions.
//!
//! See [field mapping](../../docs/plans/2026-05-04-api-converter-plan/field-mapping.md)
//! and [error model](../../docs/plans/2026-05-04-api-converter-plan/error-model.md)
//! for detailed specifications.

#![allow(dead_code)]

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Supported API types for protocol conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiType {
    /// Anthropic Claude API (/v1/messages)
    Anthropic,
    /// OpenAI Chat Completions API (/v1/chat/completions)
    OpenAIChat,
    /// OpenAI Completions API (legacy /v1/completions)
    OpenAICompletions,
    /// OpenAI Responses API (/v1/responses)
    OpenAIResponses,
    /// Gemini API (generateContent)
    Gemini,
    /// Unknown/unsupported API type
    Unknown,
}

impl fmt::Display for ApiType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiType::Anthropic => write!(f, "Anthropic"),
            ApiType::OpenAIChat => write!(f, "OpenAIChat"),
            ApiType::OpenAICompletions => write!(f, "OpenAICompletions"),
            ApiType::OpenAIResponses => write!(f, "OpenAIResponses"),
            ApiType::Gemini => write!(f, "Gemini"),
            ApiType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::str::FromStr for ApiType {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Anthropic" => Ok(ApiType::Anthropic),
            "OpenAIChat" => Ok(ApiType::OpenAIChat),
            "OpenAICompletions" => Ok(ApiType::OpenAICompletions),
            "OpenAIResponses" => Ok(ApiType::OpenAIResponses),
            "Gemini" => Ok(ApiType::Gemini),
            _ => Err(ConversionError::new(
                ConversionErrorKind::SchemaMismatch,
                "bad_type",
                format!("unknown ApiType: {}", s),
            )),
        }
    }
}

/// Error kind for conversion failures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionErrorKind {
    /// Body is not valid JSON
    InvalidJson,
    /// JSON is valid but missing required fields or type mismatch
    SchemaMismatch,
    /// Recognized feature but not currently supported (e.g., audio)
    UnsupportedFeature,
    /// SSE parsing or protocol error
    StreamProtocol,
    /// Internal converter bug (assertion failure, etc.)
    Internal,
    /// Placeholder implementation (not yet implemented)
    NotImplemented,
}

impl ConversionErrorKind {
    /// Returns the short error code for logging and response headers
    /// as specified in error-model.md §2
    pub fn short_code(&self) -> &'static str {
        match self {
            ConversionErrorKind::InvalidJson => "invalid_json",
            ConversionErrorKind::SchemaMismatch => "schema_mismatch",
            ConversionErrorKind::UnsupportedFeature => "unsupported_feature",
            ConversionErrorKind::StreamProtocol => "sse_parse",
            ConversionErrorKind::Internal => "internal",
            ConversionErrorKind::NotImplemented => "not_implemented",
        }
    }
}

/// Target API quirks that affect conversion behavior
/// 
/// In P2, this is a placeholder structure. In P7, these will be
/// populated from provider configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetQuirks {
    /// Whether to use max_completion_tokens instead of max_tokens
    /// (for reasoning models like o1)
    pub use_max_completion_tokens: bool,
}

/// Conversion error with detailed context
#[derive(Debug, Clone)]
pub struct ConversionError {
    /// Short error description, e.g., "missing_field: messages"
    pub message: String,
    /// Field path where error occurred, e.g., "request.messages[2].content[0].type"
    pub field_path: Option<String>,
    /// Original JSON value of the problematic field (truncated to 1KB)
    pub original_value: Option<Value>,
    /// Complete original body (truncated to 4KB) for debugging
    pub original_body: Option<Bytes>,
    /// Error classification
    pub kind: ConversionErrorKind,
    /// Short error code as specified in error-model.md §2
    pub code: &'static str,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.message, self.code)
    }
}

impl std::error::Error for ConversionError {}

impl ConversionError {
    /// Creates a new conversion error
    pub fn new(kind: ConversionErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            field_path: None,
            original_value: None,
            original_body: None,
            kind,
            code,
        }
    }

    /// Sets the field path for this error
    pub fn with_field_path(mut self, path: impl Into<String>) -> Self {
        self.field_path = Some(path.into());
        self
    }

    /// Sets the original value for this error
    pub fn with_original_value(mut self, value: Value) -> Self {
        self.original_value = Some(value);
        self
    }

    /// Sets the original body for this error
    pub fn with_original_body(mut self, body: Bytes) -> Self {
        self.original_body = Some(truncate_bytes_utf8(&body, 4096));
        self
    }
}

/// Trait for API protocol converters
pub trait ApiConverter: Send + Sync {
    /// Converts a request body from source format to target format
    ///
    /// # Arguments
    /// * `body` - Raw request body bytes
    /// * `target` - Target API type to convert to
    ///
    /// # Returns
    /// Converted body bytes or conversion error
    fn convert_request(&self, body: &Bytes, target: ApiType) -> Result<Bytes, ConversionError>;

    /// Converts a response body from source format to target format
    ///
    /// # Arguments
    /// * `body` - Raw response body bytes
    /// * `source` - Source API type of the response
    /// * `target` - Target API type to convert to
    /// * `is_stream` - Whether the response is streaming (SSE)
    ///
    /// # Returns
    /// Converted body bytes or conversion error
    fn convert_response(
        &self,
        body: &Bytes,
        source: ApiType,
        target: ApiType,
        is_stream: bool,
    ) -> Result<Bytes, ConversionError>;
}

pub mod anthropic_to_openai;
pub mod known_fields;
pub mod openai_to_anthropic;
pub mod stream;
pub mod telemetry;
pub use anthropic_to_openai::AnthropicToOpenAIConverter;
pub use openai_to_anthropic::OpenAIChatToAnthropicConverter;
pub use stream::StreamConverter;
pub use telemetry::{TelemetryContext, NoopTelemetry};


/// Maps an inbound path to the target provider path based on source and target API types
///
/// # Arguments
/// * `source` - Source API type
/// * `target` - Target API type
/// * `inbound_path` - The inbound request path
///
/// # Returns
/// Target path or error if mapping not supported
pub fn target_path(source: ApiType, target: ApiType, inbound_path: &str) -> Result<String, ConversionError> {
    match (source, target, inbound_path) {
        // Anthropic → OpenAI Chat: /a2o/v1/messages → /v1/chat/completions
        (ApiType::Anthropic, ApiType::OpenAIChat, "/a2o/v1/messages") => {
            Ok("/v1/chat/completions".to_string())
        }
        // OpenAI Chat → Anthropic: /o2a/v1/chat/completions → /v1/messages
        (ApiType::OpenAIChat, ApiType::Anthropic, "/o2a/v1/chat/completions") => {
            Ok("/v1/messages".to_string())
        }
        // Other paths under conversion prefixes are not supported yet
        (ApiType::Anthropic, ApiType::OpenAIChat, _) if inbound_path.starts_with("/a2o/v1/") => {
            Err(ConversionError::new(
                ConversionErrorKind::UnsupportedFeature,
                "unsupported_feature",
                format!("Unsupported path for Anthropic→OpenAI conversion: {}", inbound_path),
            ))
        }
        (ApiType::OpenAIChat, ApiType::Anthropic, _) if inbound_path.starts_with("/o2a/v1/") => {
            Err(ConversionError::new(
                ConversionErrorKind::UnsupportedFeature,
                "unsupported_feature",
                format!("Unsupported path for OpenAI→Anthropic conversion: {}", inbound_path),
            ))
        }
        // Other combinations not supported
        _ => Err(ConversionError::new(
            ConversionErrorKind::UnsupportedFeature,
            "unsupported_feature",
            format!("Unsupported conversion path: {:?} → {:?} → {}", source, target, inbound_path),
        )),
    }
}

/// Safely truncates bytes to a maximum length while respecting UTF-8 character boundaries
///
/// This function ensures that truncation never splits a multi-byte UTF-8 character,
/// which is especially important for 4-byte emoji characters that may span across
/// the truncation boundary.
///
/// # Arguments
/// * `data` - Input bytes
/// * `max` - Maximum length in bytes
///
/// # Returns
/// Truncated bytes as a `Bytes` object
///
/// # Examples
/// ```
/// use zz::converter::truncate_bytes_utf8;
/// let data = b"Hello, world!";
/// let truncated = truncate_bytes_utf8(data, 8);
/// assert_eq!(&*truncated, b"Hello, w");
/// ```
pub fn truncate_bytes_utf8(data: &[u8], max: usize) -> Bytes {
    if data.len() <= max {
        return Bytes::copy_from_slice(data);
    }

    // Find the last valid UTF-8 boundary before or at max
    let mut end = max;
    while end > 0 {
        if std::str::from_utf8(&data[..end]).is_ok() {
            break;
        }
        end -= 1;
    }

    // If we couldn't find a valid boundary, return empty
    if end == 0 {
        return Bytes::new();
    }

    Bytes::copy_from_slice(&data[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_type_display_and_from_str_roundtrip() {
        let types = [
            ApiType::Anthropic,
            ApiType::OpenAIChat,
            ApiType::OpenAICompletions,
            ApiType::OpenAIResponses,
            ApiType::Gemini,
        ];

        for api_type in types {
            let s = api_type.to_string();
            let parsed: ApiType = s.parse().unwrap();
            assert_eq!(api_type, parsed, "Roundtrip failed for {}", s);
        }
    }

    #[test]
    fn api_type_from_str_rejects_invalid() {
        let result: Result<ApiType, _> = "InvalidType".parse();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, ConversionErrorKind::SchemaMismatch);
        assert_eq!(err.code, "bad_type");
    }

    #[test]
    fn conversion_error_builder_methods_work() {
        let error = ConversionError::new(ConversionErrorKind::SchemaMismatch, "missing_field", "test error")
            .with_field_path("request.messages")
            .with_original_value(serde_json::json!("test"))
            .with_original_body(Bytes::from("test body"));

        assert_eq!(error.message, "test error");
        assert_eq!(error.code, "missing_field");
        assert_eq!(error.field_path, Some("request.messages".to_string()));
        assert_eq!(error.original_value, Some(serde_json::json!("test")));
        assert!(error.original_body.is_some());
        assert_eq!(error.kind, ConversionErrorKind::SchemaMismatch);
    }

    #[test]
    fn truncate_bytes_utf8_handles_simple_ascii() {
        let data = b"Hello world";
        let truncated = truncate_bytes_utf8(data, 5);
        assert_eq!(truncated.as_ref(), b"Hello");
    }

    #[test]
    fn truncate_bytes_utf8_handles_shorter_input() {
        let data = b"Hello";
        let truncated = truncate_bytes_utf8(data, 10);
        assert_eq!(truncated.as_ref(), data);
    }

    #[test]
    fn truncate_bytes_utf8_handles_2byte_utf8() {
        // "café" has é as 2-byte UTF-8 (0xC3 0xA9)
        let data = "café".as_bytes();
        let truncated = truncate_bytes_utf8(data, 4);
        assert_eq!(truncated.as_ref(), "caf".as_bytes());
    }

    #[test]
    fn truncate_bytes_utf8_handles_3byte_utf8() {
        // "你好" are 3-byte UTF-8 characters
        let data = "你好世界".as_bytes();
        let truncated = truncate_bytes_utf8(data, 5);
        assert_eq!(truncated.as_ref(), "你".as_bytes());
    }

    #[test]
    fn truncate_bytes_utf8_handles_4byte_emoji_boundary() {
        // 🌍 is a 4-byte UTF-8 character (0xF0 0x9F 0x8C 0x8D)
        let data = "Hello 🌍 world".as_bytes();
        // "Hello " is 6 bytes, 🌍 starts at byte 6
        // Truncating at 8 should cut before the emoji completes
        let truncated = truncate_bytes_utf8(data, 8);
        assert_eq!(truncated.as_ref(), "Hello ".as_bytes());
    }

    #[test]
    fn truncate_bytes_utf8_handles_emoji_at_boundary() {
        // Multiple emojis in sequence
        let data = "🌍🌎🌏".as_bytes(); // Each emoji is 4 bytes
        let truncated = truncate_bytes_utf8(data, 6);
        // Should include exactly one emoji (4 bytes) and stop before the second
        assert_eq!(truncated.as_ref(), "🌍".as_bytes());
    }

    #[test]
    fn truncate_bytes_utf8_handles_empty_input() {
        let data = b"";
        let truncated = truncate_bytes_utf8(data, 10);
        assert!(truncated.is_empty());
    }

    #[test]
    fn truncate_bytes_utf8_handles_zero_max() {
        let data = b"Hello";
        let truncated = truncate_bytes_utf8(data, 0);
        assert!(truncated.is_empty());
    }

    #[test]
    fn not_implemented_returns_expected_kind() {
        // Request conversion is now implemented for OpenAIChatToAnthropicConverter
        // Response conversion is implemented for non-streaming
        // Streaming should still return NotImplemented
        let converter = OpenAIChatToAnthropicConverter;
        let result = converter.convert_response(&Bytes::new(), ApiType::Anthropic, ApiType::OpenAIChat, true);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            ConversionErrorKind::NotImplemented
        );

        // AnthropicToOpenAIConverter response streaming should also return NotImplemented
        let converter = AnthropicToOpenAIConverter;
        let result = converter.convert_response(&Bytes::new(), ApiType::Anthropic, ApiType::OpenAIChat, true);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind,
            ConversionErrorKind::NotImplemented
        );
    }

    #[test]
    fn target_path_mappings_work() {
        // Test the two implemented mappings
        let result = target_path(ApiType::Anthropic, ApiType::OpenAIChat, "/a2o/v1/messages");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/v1/chat/completions");

        let result = target_path(ApiType::OpenAIChat, ApiType::Anthropic, "/o2a/v1/chat/completions");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/v1/messages");

        // Test unsupported paths return unsupported_feature
        let result = target_path(ApiType::Anthropic, ApiType::OpenAIChat, "/a2o/v1/models");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "unsupported_feature");

        let result = target_path(ApiType::OpenAIChat, ApiType::Anthropic, "/o2a/v1/models");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "unsupported_feature");

        // Test unsupported combinations
        let result = target_path(ApiType::Anthropic, ApiType::Anthropic, "/v1/messages");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "unsupported_feature");
    }
}
