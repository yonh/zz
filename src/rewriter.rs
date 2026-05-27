pub struct RequestRewriter;

impl RequestRewriter {
    /// Rewrites request URL by joining the provider base URL and incoming path.
    pub fn rewrite_url(base_url: &str, path: &str) -> Result<String, anyhow::Error> {
        // Ensure base_url has trailing slash so url::Url::join doesn't strip
        // the last path segment. e.g. "https://host/apps/anthropic" + "/v1/messages"
        // would otherwise produce "https://host/apps/v1/messages".
        let base = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{}/", base_url)
        };
        let url = url::Url::parse(&base)?;

        // Avoid duplicating path segments when base_url already contains a prefix
        // that overlaps with the start of path.
        // e.g. base_url="https://host/v1" + path="/v1/chat/completions"
        //      should produce "https://host/v1/chat/completions", not ".../v1/v1/..."
        let base_path = url.path().trim_end_matches('/');
        let trimmed_path = path.trim_start_matches('/');

        // Find the last segment of base_url path and check if trimmed_path starts with it
        let effective_path = if let Some(last_slash) = base_path.rfind('/') {
            let base_suffix = &base_path[last_slash + 1..];
            if !base_suffix.is_empty() && trimmed_path.starts_with(base_suffix) {
                let rest = &trimmed_path[base_suffix.len()..];
                if rest.is_empty() || rest.starts_with('/') {
                    // Skip the overlapping prefix; use the rest (strip leading '/')
                    rest.trim_start_matches('/')
                } else {
                    trimmed_path
                }
            } else {
                trimmed_path
            }
        } else {
            trimmed_path
        };

        let joined = url.join(effective_path)?;
        Ok(joined.to_string())
    }

    pub fn rewrite_headers(
        headers: &hyper::HeaderMap,
        api_key: &str,
        base_url: &str,
        custom_headers: &std::collections::HashMap<String, String>,
    ) -> hyper::HeaderMap {
        let mut new_headers = headers.clone();

        // Remove inbound authentication headers to avoid leaking client keys upstream
        // and accidentally overriding provider credentials.
        new_headers.remove(hyper::header::AUTHORIZATION);
        new_headers.remove(hyper::header::PROXY_AUTHORIZATION);
        new_headers.remove(hyper::header::HeaderName::from_static("x-api-key"));
        new_headers.remove(hyper::header::HeaderName::from_static("api-key"));
        new_headers.remove(hyper::header::HeaderName::from_static("api_key"));

        // Set Authorization
        new_headers.insert(
            hyper::header::AUTHORIZATION,
            format!("Bearer {}", api_key).parse().unwrap(),
        );

        // Set Host from base_url
        if let Ok(url) = url::Url::parse(base_url) {
            if let Some(host) = url.host_str() {
                new_headers.insert(
                    hyper::header::HOST,
                    host.parse().unwrap(),
                );
            }
        }

        // Add custom headers
        for (key, value) in custom_headers {
            let header_name: hyper::header::HeaderName = key.as_str().parse().unwrap();
            let header_value: hyper::header::HeaderValue = value.parse().unwrap();
            new_headers.insert(header_name, header_value);
        }

        new_headers
    }
}

#[cfg(test)]
mod tests {
    use super::RequestRewriter;

    /// Ensures inbound auth headers are removed and provider API key is used.
    #[test]
    fn rewrite_headers_replaces_inbound_auth_headers() {
        let mut inbound = hyper::HeaderMap::new();
        inbound.insert(hyper::header::AUTHORIZATION, "Bearer inbound-key".parse().unwrap());
        inbound.insert(hyper::header::HeaderName::from_static("x-api-key"), "inbound-x".parse().unwrap());
        inbound.insert(hyper::header::HeaderName::from_static("api-key"), "inbound-api".parse().unwrap());
        inbound.insert(hyper::header::HeaderName::from_static("api_key"), "inbound_api".parse().unwrap());
        inbound.insert(hyper::header::CONTENT_TYPE, "application/json".parse().unwrap());

        let rewritten = RequestRewriter::rewrite_headers(
            &inbound,
            "provider-key",
            "https://example.com/v1",
            &std::collections::HashMap::new(),
        );

        assert_eq!(
            rewritten.get(hyper::header::AUTHORIZATION).unwrap(),
            "Bearer provider-key"
        );
        assert!(rewritten.get(hyper::header::HeaderName::from_static("x-api-key")).is_none());
        assert!(rewritten.get(hyper::header::HeaderName::from_static("api-key")).is_none());
        assert!(rewritten.get(hyper::header::HeaderName::from_static("api_key")).is_none());
        assert_eq!(rewritten.get(hyper::header::CONTENT_TYPE).unwrap(), "application/json");
    }

    /// Ensures overlapping path prefix is deduplicated
    #[test]
    fn rewrite_url_deduplicates_overlapping_prefix() {
        // base_url ends with /v1, path starts with /v1/...
        let result = RequestRewriter::rewrite_url(
            "https://host.example.com/v1",
            "/v1/chat/completions",
        ).unwrap();
        assert_eq!(result, "https://host.example.com/v1/chat/completions");

        // base_url without /v1, path has /v1/... — no dedup needed
        let result = RequestRewriter::rewrite_url(
            "https://host.example.com",
            "/v1/chat/completions",
        ).unwrap();
        assert_eq!(result, "https://host.example.com/v1/chat/completions");

        // base_url ends with /v1/, path is /v1/messages
        let result = RequestRewriter::rewrite_url(
            "https://host.example.com/v1/",
            "/v1/messages",
        ).unwrap();
        assert_eq!(result, "https://host.example.com/v1/messages");
    }

    /// Ensures configured custom headers can still be explicitly injected.
    #[test]
    fn rewrite_headers_keeps_custom_header_injection() {
        let inbound = hyper::HeaderMap::new();
        let mut custom = std::collections::HashMap::new();
        custom.insert("x-api-key".to_string(), "provider-x-key".to_string());

        let rewritten = RequestRewriter::rewrite_headers(
            &inbound,
            "provider-key",
            "https://example.com/v1",
            &custom,
        );

        assert_eq!(
            rewritten.get(hyper::header::AUTHORIZATION).unwrap(),
            "Bearer provider-key"
        );
        assert_eq!(
            rewritten
                .get(hyper::header::HeaderName::from_static("x-api-key"))
                .unwrap(),
            "provider-x-key"
        );
    }
}
