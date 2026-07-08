use std::time::Instant;

use axum::{
    extract::Request,
    http::{HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::{error, info, warn};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub async fn log_request(request: Request, next: Next) -> Response {
    let request_id = request_id(request.headers().get(REQUEST_ID_HEADER));
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let started_at = Instant::now();

    let mut response = next.run(request).await;
    let status = response.status();
    let latency_ms = elapsed_millis(started_at);

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    log_completed_request(status, &request_id, &method, &path, latency_ms);
    response
}

fn request_id(value: Option<&HeaderValue>) -> String {
    value
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn elapsed_millis(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn log_completed_request(
    status: StatusCode,
    request_id: &str,
    method: &Method,
    path: &str,
    latency_ms: u64,
) {
    let status_code = status.as_u16();

    if status.is_server_error() {
        error!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = status_code,
            latency_ms,
            "HTTP request failed"
        );
    } else if status.is_client_error() {
        warn!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = status_code,
            latency_ms,
            "HTTP request rejected"
        );
    } else {
        info!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = status_code,
            latency_ms,
            "HTTP request completed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn,
        routing::get,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn adds_request_id_header_to_response() {
        let response = test_app(StatusCode::NO_CONTENT)
            .oneshot(request("/health", None))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response.headers().contains_key(REQUEST_ID_HEADER),
            "response should include request id"
        );
    }

    #[tokio::test]
    async fn preserves_existing_request_id_header() {
        let response = test_app(StatusCode::OK)
            .oneshot(request("/health?ignored=true", Some("debug-request-1")))
            .await
            .expect("request should complete");

        assert_eq!(
            response.headers().get(REQUEST_ID_HEADER),
            Some(&HeaderValue::from_static("debug-request-1"))
        );
    }

    #[tokio::test]
    async fn keeps_error_status_unchanged() {
        let response = test_app(StatusCode::INTERNAL_SERVER_ERROR)
            .oneshot(request("/health", None))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(response.headers().contains_key(REQUEST_ID_HEADER));
    }

    fn test_app(status: StatusCode) -> Router {
        Router::new()
            .route("/health", get(move || async move { status }))
            .layer(from_fn(log_request))
    }

    fn request(path: &str, request_id: Option<&'static str>) -> Request<Body> {
        let mut builder = Request::builder().uri(path);
        if let Some(request_id) = request_id {
            builder = builder.header(REQUEST_ID_HEADER, request_id);
        }
        builder
            .body(Body::empty())
            .expect("test request should be valid")
    }
}
