use hyper::header::{ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, HeaderValue};

/// Add CORS headers to any response
pub fn add_cors_headers(resp: &mut hyper::Response<impl hyper::body::Body>) {
    let headers = resp.headers_mut();
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"));
    headers.insert(ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("Content-Type, Authorization, X-Requested-With"));
    headers.insert(ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
}

/// Create a preflight OPTIONS response
pub fn preflight_response() -> hyper::Response<http_body_util::Full<hyper::body::Bytes>> {
    use http_body_util::Full;
    let mut resp = hyper::Response::new(Full::new(hyper::body::Bytes::new()));
    add_cors_headers(&mut resp);
    *resp.status_mut() = hyper::StatusCode::NO_CONTENT;
    resp
}
