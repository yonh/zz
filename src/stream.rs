// Stream utilities for SSE support

pub fn is_sse_request(req: &hyper::Request<hyper::body::Incoming>) -> bool {
    // Check Accept header
    if let Some(accept) = req.headers().get(hyper::header::ACCEPT) {
        if let Ok(accept_str) = accept.to_str() {
            if accept_str.contains("text/event-stream") {
                return true;
            }
        }
    }
    false
}

/// Detect whether the request body contains `"stream": true`.
///
/// LLM clients (Codex, Cursor, etc.) often omit the `Accept: text/event-stream`
/// header and instead signal streaming via the JSON body field. This performs a
/// lightweight text search rather than a full JSON parse.
pub fn is_streaming_body(body: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(body) else {
        return false;
    };
    let s_lower = s.to_lowercase();
    if let Some(idx) = s_lower.find("\"stream\"") {
        let after = &s_lower[idx + 8..];
        let trimmed = after.trim_start();
        if trimmed.starts_with(':') {
            let after_colon = trimmed[1..].trim_start();
            if after_colon.starts_with("true") {
                return true;
            }
        }
    }
    false
}