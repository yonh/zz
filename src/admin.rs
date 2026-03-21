use std::sync::Arc;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::Bytes;

type ResponseBody = BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> ResponseBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

pub fn handle_admin_request(
    req: &hyper::Request<hyper::body::Incoming>,
    state: Option<&super::proxy::AppState>,
) -> Option<hyper::Response<ResponseBody>> {
    let path = req.uri().path();

    if !path.starts_with("/zz/") {
        return None;
    }

    match (path, req.method()) {
        ("/zz/health", &hyper::Method::GET) => Some(handle_health(state)),
        ("/zz/stats", &hyper::Method::GET) => Some(handle_stats(state)),
        ("/zz/reload", &hyper::Method::POST) => Some(handle_reload(state)),
        _ => {
            let mut resp = hyper::Response::new(full("Not found"));
            *resp.status_mut() = hyper::StatusCode::NOT_FOUND;
            Some(resp)
        }
    }
}

fn handle_health(state: Option<&super::proxy::AppState>) -> hyper::Response<ResponseBody> {
    let body = if let Some(s) = state {
        let providers = s.provider_manager.get_all_states();
        serde_json::to_string(&serde_json::json!({
            "status": "ok",
            "providers": providers
        })).unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
    } else {
        "{\"status\":\"ok\"}".to_string()
    };

    let mut resp = hyper::Response::new(full(body));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}

fn handle_stats(_state: Option<&super::proxy::AppState>) -> hyper::Response<ResponseBody> {
    // TODO: Implement actual stats tracking
    let body = serde_json::to_string(&serde_json::json!({
        "requests": 0,
        "errors": 0,
        "providers": []
    })).unwrap();

    let mut resp = hyper::Response::new(full(body));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}

fn handle_reload(_state: Option<&super::proxy::AppState>) -> hyper::Response<ResponseBody> {
    // TODO: Implement actual config reload
    let body = serde_json::to_string(&serde_json::json!({
        "reloaded": false,
        "message": "Not implemented yet"
    })).unwrap();

    let mut resp = hyper::Response::new(full(body));
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    resp
}