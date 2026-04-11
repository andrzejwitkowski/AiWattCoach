use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, Uri};
use http_body_util::BodyExt;
use tower::{Layer, Service};

use super::redaction::{
    format_binary_body_preview, format_body_preview, redact_headers, redact_value,
};
use super::EndpointLogConfig;

/// Tower Layer that logs request and response bodies with redaction.
#[derive(Clone, Default)]
pub struct RequestLogLayer;

impl RequestLogLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for RequestLogLayer {
    type Service = RequestLogService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestLogService { inner }
    }
}

/// Tower Service that buffers and logs request/response bodies.
#[derive(Clone)]
pub struct RequestLogService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RequestLogService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Display + std::fmt::Debug,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let config = req
                .extensions()
                .get::<EndpointLogConfig>()
                .cloned()
                .unwrap_or_default();

            if !config.log_request_body && !config.log_response_body {
                return inner.call(req).await;
            }

            let method = req.method().clone();
            let uri = req.uri().clone();
            let headers = req.headers().clone();

            let rebuilt_req = if config.log_request_body {
                let (parts, body) = req.into_parts();
                let body_bytes = collect_body(body).await;
                let body_preview = body_bytes.as_ref().map(|bytes| {
                    format_body_for_logging(&method, &headers, bytes, config.max_body_bytes)
                });
                log_request(&method, &uri, &headers, body_preview.as_deref());
                Request::from_parts(parts, Body::from(body_bytes.unwrap_or_default()))
            } else {
                req
            };

            let response = inner.call(rebuilt_req).await?;

            if !config.log_response_body {
                return Ok(response);
            }

            let (resp_parts, resp_body) = response.into_parts();
            let resp_body_bytes = collect_body(resp_body).await;
            let resp_preview = resp_body_bytes.as_ref().map(|bytes| {
                format_response_body_for_logging(&resp_parts.headers, bytes, config.max_body_bytes)
            });

            log_response(resp_parts.status, resp_preview.as_deref());

            Ok(Response::from_parts(
                resp_parts,
                Body::from(resp_body_bytes.unwrap_or_default()),
            ))
        })
    }
}

/// Collect the full body for logging and response reconstruction.
async fn collect_body(body: Body) -> Option<Vec<u8>> {
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return None,
    };

    if bytes.is_empty() {
        return None;
    }

    Some(bytes.to_vec())
}

/// Format request body for logging with redaction.
fn format_body_for_logging(
    method: &Method,
    headers: &axum::http::HeaderMap,
    bytes: &[u8],
    max_body_bytes: usize,
) -> String {
    if !matches!(method, &Method::POST | &Method::PUT | &Method::PATCH) {
        return "(no body for this method)".to_string();
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_json = content_type.contains("application/json");
    let is_textual = content_type.contains("application/x-www-form-urlencoded")
        || content_type.contains("text/")
        || is_json;

    if !is_textual {
        return format_binary_body_preview(&bytes[..bytes.len().min(max_body_bytes)]);
    }

    if !is_json {
        return summarize_text_body(content_type, bytes, max_body_bytes);
    }

    let body_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return format_binary_body_preview(&bytes[..bytes.len().min(max_body_bytes)]),
    };

    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
        redact_value(&mut json_value);
        return format_body_preview(&json_value.to_string(), 1024);
    }

    format_body_preview(body_str, 512)
}

/// Format response body for logging with redaction.
fn format_response_body_for_logging(
    headers: &axum::http::HeaderMap,
    bytes: &[u8],
    max_body_bytes: usize,
) -> String {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_json = content_type.contains("application/json")
        || content_type.contains("application/problem+json");
    let is_textual =
        content_type.contains("text/") || content_type.contains("application/xml") || is_json;

    if !is_textual {
        return format_binary_body_preview(&bytes[..bytes.len().min(max_body_bytes)]);
    }

    if !is_json {
        return summarize_text_body(content_type, bytes, max_body_bytes);
    }

    let body_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return format_binary_body_preview(&bytes[..bytes.len().min(max_body_bytes)]),
    };

    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
        redact_value(&mut json_value);
        return format_body_preview(&json_value.to_string(), 1024);
    }

    format_body_preview(body_str, 512)
}

fn summarize_text_body(content_type: &str, bytes: &[u8], max_body_bytes: usize) -> String {
    let preview_bytes = &bytes[..bytes.len().min(max_body_bytes)];
    let preview = format_binary_body_preview(preview_bytes);

    if content_type.is_empty() {
        format!(
            "textual({},content_type=unknown,total_bytes={})",
            preview,
            bytes.len()
        )
    } else {
        format!(
            "textual({},content_type={content_type},total_bytes={})",
            preview,
            bytes.len()
        )
    }
}

/// Log request details at INFO level.
fn log_request(
    method: &Method,
    uri: &Uri,
    headers: &axum::http::HeaderMap,
    body_preview: Option<&str>,
) {
    let redacted = redact_headers(headers);
    let header_fields: Vec<(&str, &str)> = redacted
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();

    if let Some(body) = body_preview {
        tracing::info!(
            http.method = %method,
            http.target = %uri.path(),
            http.headers = ?header_fields,
            request_body = body,
            "incoming request"
        );
    } else {
        tracing::info!(
            http.method = %method,
            http.target = %uri.path(),
            http.headers = ?header_fields,
            "incoming request (no body)"
        );
    }
}

/// Log response details at level matching status.
fn log_response(status: StatusCode, body_preview: Option<&str>) {
    if let Some(body) = body_preview {
        if status.is_server_error() {
            tracing::event!(
                tracing::Level::ERROR,
                http.status_code = status.as_u16(),
                response_body = body,
                "outgoing response"
            );
        } else if status.is_client_error() {
            tracing::event!(
                tracing::Level::WARN,
                http.status_code = status.as_u16(),
                response_body = body,
                "outgoing response"
            );
        } else {
            tracing::event!(
                tracing::Level::INFO,
                http.status_code = status.as_u16(),
                response_body = body,
                "outgoing response"
            );
        }
    } else if status.is_server_error() {
        tracing::event!(
            tracing::Level::ERROR,
            http.status_code = status.as_u16(),
            "outgoing response (no body)"
        );
    } else if status.is_client_error() {
        tracing::event!(
            tracing::Level::WARN,
            http.status_code = status.as_u16(),
            "outgoing response (no body)"
        );
    } else {
        tracing::event!(
            tracing::Level::INFO,
            http.status_code = status.as_u16(),
            "outgoing response (no body)"
        );
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        middleware::Next,
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use tower::util::ServiceExt;

    use crate::adapters::rest::logging::EndpointLogConfig;

    use super::{format_body_for_logging, format_response_body_for_logging, RequestLogLayer};

    const TEST_BODY_LIMIT: usize = 64 * 1024;

    async fn insert_test_log_config(mut req: Request<Body>, next: Next) -> Response {
        req.extensions_mut()
            .insert(EndpointLogConfig::full().with_max_body_bytes(8));
        next.run(req).await
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preserves_large_request_body_when_body_logging_enabled() {
        async fn echo(body: String) -> impl IntoResponse {
            body
        }

        let app = Router::new()
            .route("/echo", post(echo))
            .layer(RequestLogLayer::new())
            .layer(axum::middleware::from_fn(insert_test_log_config));

        let body = "abcdefghij".repeat(100);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .header("content-type", "text/plain")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let bytes = to_bytes(response.into_body(), TEST_BODY_LIMIT)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(bytes.to_vec()).unwrap(), body);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preserves_large_response_body_when_body_logging_enabled() {
        async fn large_response() -> impl IntoResponse {
            "abcdefghij".repeat(100)
        }

        let app = Router::new()
            .route("/response", post(large_response))
            .layer(RequestLogLayer::new())
            .layer(axum::middleware::from_fn(insert_test_log_config));

        let expected = "abcdefghij".repeat(100);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/response")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let bytes = to_bytes(response.into_body(), TEST_BODY_LIMIT)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(bytes.to_vec()).unwrap(), expected);
    }

    #[test]
    fn request_logging_summarizes_form_encoded_bodies_without_raw_content() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "content-type",
            "application/x-www-form-urlencoded".parse().unwrap(),
        );

        let preview = format_body_for_logging(
            &axum::http::Method::POST,
            &headers,
            b"username=alice&password=super-secret",
            1024,
        );

        assert!(preview.contains("textual("));
        assert!(preview.contains("content_type=application/x-www-form-urlencoded"));
        assert!(!preview.contains("password=super-secret"));
        assert!(!preview.contains("alice"));
    }

    #[test]
    fn response_logging_summarizes_text_bodies_without_raw_content() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("content-type", "text/plain".parse().unwrap());

        let preview =
            format_response_body_for_logging(&headers, b"token=super-secret-response", 1024);

        assert!(preview.contains("textual("));
        assert!(preview.contains("content_type=text/plain"));
        assert!(!preview.contains("super-secret-response"));
    }
}
