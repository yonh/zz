pub struct RequestRewriter;

impl RequestRewriter {
    pub fn rewrite_url(base_url: &str, path: &str) -> Result<String, anyhow::Error> {
        let url = url::Url::parse(base_url)?;
        let joined = url.join(path.trim_start_matches('/'))?;
        // Preserve query and fragment from original if needed
        Ok(joined.to_string())
    }

    pub fn rewrite_headers(
        headers: &hyper::HeaderMap,
        api_key: &str,
        base_url: &str,
        custom_headers: &std::collections::HashMap<String, String>,
    ) -> hyper::HeaderMap {
        let mut new_headers = headers.clone();

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
