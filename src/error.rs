use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("Config error: {0}")]
    ConfigError(#[from] anyhow::Error),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Quota exhausted for provider: {0}")]
    QuotaExhausted(String),

    #[error("All providers failed: {0:?}")]
    AllProvidersFailed(Vec<ProxyError>),

    #[error("HTTP error: {0}")]
    HttpError(String),
}

/// Check if response indicates quota exhaustion
/// - HTTP 429 (Too Many Requests)
/// - HTTP 403 with quota-related body keywords
pub fn is_quota_error(status: hyper::StatusCode, body: &[u8]) -> bool {
    if status == hyper::StatusCode::TOO_MANY_REQUESTS {
        return true;
    }

    if status == hyper::StatusCode::FORBIDDEN {
        // Only check first 1KB of body as per spec
        let check_len = body.len().min(1024);
        let body_slice = &body[..check_len];
        let body_str = std::str::from_utf8(body_slice).unwrap_or("");
        let body_lower = body_str.to_lowercase();
        let keywords = [
            "quota", "rate limit", "exceeded", "insufficient_quota",
            "billing", "limit reached",
        ];
        return keywords.iter().any(|kw| body_lower.contains(kw));
    }

    false
}