// Stream utilities for SSE support
// Currently SSE detection is handled in proxy.rs
// This module is reserved for future streaming utilities

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