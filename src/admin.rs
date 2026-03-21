pub fn handle_admin_request(
    req: &hyper::Request<hyper::body::Incoming>,
) -> Option<hyper::Response<http_body_util::Full<bytes::Bytes>>> {
    let path = req.uri().path();

    if !path.starts_with("/zz/") {
        return None;
    }

    match (path, req.method()) {
        ("/zz/health", &hyper::Method::GET) => Some(handle_health()),
        ("/zz/stats", &hyper::Method::GET) => Some(handle_stats()),
        ("/zz/reload", &hyper::Method::POST) => Some(handle_reload()),
        _ => {
            let body = http_body_util::Full::from("Not found");
            let mut resp = hyper::Response::new(body);
            *resp.status_mut() = hyper::StatusCode::NOT_FOUND;
            Some(resp)
        }
    }
}

fn handle_health() -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
    let body = http_body_util::Full::from("{\"status\":\"ok\"}");
    let mut resp = hyper::Response::new(body);
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}

fn handle_stats() -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
    let body = http_body_util::Full::from("{\"requests\":0}");
    let mut resp = hyper::Response::new(body);
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}

fn handle_reload() -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
    let body = http_body_util::Full::from("{\"reloaded\":true}");
    let mut resp = hyper::Response::new(body);
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}
