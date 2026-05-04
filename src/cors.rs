use hyper::header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue};

/// Check if a request origin matches the allowed origins list.
///
/// Supports wildcard patterns:
/// - `"*"` matches everything
/// - `"http://localhost:*"` matches any localhost port
/// - Exact string match otherwise
pub fn is_origin_allowed(origin: &str, allowed_origins: &[String]) -> bool {
    for pattern in allowed_origins {
        if pattern == "*" {
            return true;
        }
        if pattern == origin {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix(":*") {
            // Pattern like "http://localhost:*" — match scheme + host exactly,
            // then expect a port number after the colon.
            // prefix = "http://localhost", origin must start with "http://localhost:"
            if let Some(rest) = origin.strip_prefix(prefix) {
                if let Some(port_str) = rest.strip_prefix(':') {
                    // Port must be non-empty and all digits
                    if !port_str.is_empty() && port_str.chars().all(|c| c.is_ascii_digit()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Add CORS headers to a response, using the configured allowed origins.
///
/// If `allowed_origins` is empty or contains "*", falls back to permissive "*".
/// Otherwise, echoes back the request's Origin header if it matches an allowed pattern.
pub fn add_cors_headers(
    resp: &mut hyper::Response<impl hyper::body::Body>,
    allowed_origins: &[String],
    request_origin: Option<&str>,
) {
    let headers = resp.headers_mut();

    let is_specific_origin = match request_origin {
        Some(origin) if is_origin_allowed(origin, allowed_origins) && origin != "*" => {
            headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(origin).unwrap_or_else(|_| HeaderValue::from_static("*")));
            true
        }
        _ => {
            headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
            false
        }
    };

    headers.insert(ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"));
    headers.insert(ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("Content-Type, Authorization, X-Requested-With, X-Admin-Key"));
    // Only set credentials when echoing a specific origin — browsers reject
    // Access-Control-Allow-Credentials:true with wildcard origin per Fetch spec.
    if is_specific_origin {
        headers.insert(ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
    }
}

/// Create a preflight OPTIONS response.
pub fn preflight_response(
    allowed_origins: &[String],
    request_origin: Option<&str>,
) -> hyper::Response<http_body_util::Full<hyper::body::Bytes>> {
    use http_body_util::Full;
    let mut resp = hyper::Response::new(Full::new(hyper::body::Bytes::new()));
    add_cors_headers(&mut resp, allowed_origins, request_origin);
    *resp.status_mut() = hyper::StatusCode::NO_CONTENT;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origins(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_wildcard_matches_all() {
        assert!(is_origin_allowed("http://evil.com", &origins(&["*"])));
        assert!(is_origin_allowed("https://any.host:9999", &origins(&["*"])));
    }

    #[test]
    fn test_exact_match() {
        assert!(is_origin_allowed("http://localhost:3000", &origins(&["http://localhost:3000"])));
        assert!(!is_origin_allowed("http://localhost:3001", &origins(&["http://localhost:3000"])));
    }

    #[test]
    fn test_wildcard_port_matches_valid_ports() {
        let patterns = origins(&["http://localhost:*"]);
        assert!(is_origin_allowed("http://localhost:3000", &patterns));
        assert!(is_origin_allowed("http://localhost:8080", &patterns));
        assert!(is_origin_allowed("http://localhost:1", &patterns));
    }

    #[test]
    fn test_wildcard_port_rejects_suffix_bypass() {
        let patterns = origins(&["http://localhost:*"]);
        // This was the bug: starts_with("http://localhost") matched evil.com
        assert!(!is_origin_allowed("http://localhost.evil.com", &patterns));
        assert!(!is_origin_allowed("http://localhost.attacker.com:8080", &patterns));
        assert!(!is_origin_allowed("http://localhostX", &patterns));
    }

    #[test]
    fn test_wildcard_port_rejects_non_digit_suffix() {
        let patterns = origins(&["http://localhost:*"]);
        assert!(!is_origin_allowed("http://localhost:80abc", &patterns));
        assert!(!is_origin_allowed("http://localhost:80/path", &patterns));
    }

    #[test]
    fn test_wildcard_port_no_match_different_host() {
        let patterns = origins(&["http://localhost:*"]);
        assert!(!is_origin_allowed("http://example.com:3000", &patterns));
    }
}
