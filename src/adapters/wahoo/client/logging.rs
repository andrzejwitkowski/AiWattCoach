use std::time::Instant;

use bytes::Bytes;
use reqwest::{header::HeaderMap, Method, Request, RequestBuilder, StatusCode};

use crate::telemetry::is_sensitive_key;

const CLIENT_NAME: &str = "wahoo_oauth";

pub struct LoggedResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

pub async fn execute_and_log_no_body(
    client: &reqwest::Client,
    request: RequestBuilder,
) -> Result<LoggedResponse, reqwest::Error> {
    execute_and_log_without_body(client, request.build()?).await
}

async fn execute_and_log_without_body(
    client: &reqwest::Client,
    request: Request,
) -> Result<LoggedResponse, reqwest::Error> {
    let method = request.method().clone();
    let url = request.url().clone();
    let headers = request.headers().clone();

    log_request(&method, &url, &headers);

    let start = Instant::now();
    let response = client
        .execute(request)
        .await
        .inspect_err(|error| log_transport_failure(&method, &url, error, "request_send_failed"))?;
    let latency = start.elapsed();

    let status = response.status();
    let body = response
        .bytes()
        .await
        .inspect_err(|error| log_transport_failure(&method, &url, error, "response_read_failed"))?;

    log_response(&method, &url, status, latency);

    Ok(LoggedResponse { status, body })
}

fn log_request(method: &Method, url: &reqwest::Url, headers: &HeaderMap) {
    let header_fields: Vec<(&str, String)> = headers
        .iter()
        .map(|(name, value)| {
            let value = value.to_str().unwrap_or("[binary]").to_string();
            let value = if is_sensitive_header(name.as_str()) {
                "[REDACTED]".to_string()
            } else {
                value
            };

            (name.as_str(), value)
        })
        .collect();

    tracing::info!(
        provider = CLIENT_NAME,
        http.method = %method,
        http.url = %sanitized_url(url),
        http.headers = ?header_fields,
        "outgoing request (no body)"
    );
}

fn log_response(
    method: &Method,
    url: &reqwest::Url,
    status: StatusCode,
    latency: std::time::Duration,
) {
    match response_log_level(status) {
        tracing::Level::ERROR => tracing::event!(
            tracing::Level::ERROR,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        ),
        tracing::Level::WARN => tracing::event!(
            tracing::Level::WARN,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        ),
        _ => tracing::event!(
            tracing::Level::INFO,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        ),
    }
}

fn response_log_level(status: StatusCode) -> tracing::Level {
    match status {
        status if status.is_server_error() => tracing::Level::ERROR,
        status if status.is_client_error() => tracing::Level::WARN,
        _ => tracing::Level::INFO,
    }
}

fn log_transport_failure(method: &Method, url: &reqwest::Url, error: &reqwest::Error, stage: &str) {
    tracing::error!(
        provider = CLIENT_NAME,
        http.method = %method,
        http.url = %sanitized_url(url),
        failure.stage = stage,
        error = %error,
        "outgoing request failed"
    );
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "set-cookie"
    ) || is_sensitive_key(name)
}

fn sanitized_url(url: &reqwest::Url) -> String {
    let mut sanitized = format!(
        "{}://{}{}",
        url.scheme(),
        url.host_str().unwrap_or(""),
        url.path()
    );

    if let Some(port) = url.port() {
        sanitized = format!(
            "{}://{}:{}{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            port,
            url.path()
        );
    }

    sanitized
}
