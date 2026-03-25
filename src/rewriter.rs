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
        let joined = url.join(path.trim_start_matches('/'))?;
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
