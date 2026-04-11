use std::time::Instant;

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use reqwest::{Request, RequestBuilder};
use sha2::Digest;

use crate::telemetry::is_sensitive_key;

const CLIENT_NAME: &str = "intervals_icu";

/// Maximum characters to preview in logged request/response bodies.
const MAX_LOGGED_BODY_CHARS: usize = 1024;

/// Result of a logged request — the response body is consumed and returned separately.
pub struct LoggedResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub url: reqwest::Url,
}

#[derive(Clone, Copy, Debug)]
pub enum BodyLoggingMode {
    Full,
    None,
}

pub async fn execute_request(
    client: &reqwest::Client,
    request: RequestBuilder,
    body_logging: BodyLoggingMode,
) -> Result<LoggedResponse, reqwest::Error> {
    let request = request.build()?;

    match body_logging {
        BodyLoggingMode::Full => log_request_response(client, request).await,
        BodyLoggingMode::None => log_request_response_no_body(client, request).await,
    }
}

/// Logs an outgoing request and response, consuming the response body.
///
/// The body is consumed for logging and then returned in `LoggedResponse`.
pub async fn log_request_response(
    client: &reqwest::Client,
    request: Request,
) -> Result<LoggedResponse, reqwest::Error> {
    let method = request.method().clone();
    let url = request.url().clone();
    let headers = request.headers().clone();
    let body_bytes = request
        .body()
        .and_then(|b| b.as_bytes().map(|b| b.to_vec()));

    let request_body_preview = body_bytes.as_ref().map(|b| format_request_body(&method, b));

    log_request(&method, &url, &headers, request_body_preview.as_deref());

    let start = Instant::now();
    let response = client.execute(request).await?;
    let latency = start.elapsed();

    let status = response.status();
    let resp_headers = response.headers().clone();
    let body = response.bytes().await?;

    let response_body_preview = format_response_body(&body, &resp_headers);

    log_response(&method, &url, status, latency, Some(&response_body_preview));

    Ok(LoggedResponse {
        status,
        headers: resp_headers,
        body,
        url,
    })
}

/// Logs an outgoing request and response, but does NOT log body contents.
/// Useful for large binary payloads (e.g. .fit files).
pub async fn log_request_response_no_body(
    client: &reqwest::Client,
    request: Request,
) -> Result<LoggedResponse, reqwest::Error> {
    let method = request.method().clone();
    let url = request.url().clone();
    let headers = request.headers().clone();

    log_request(&method, &url, &headers, None);

    let start = Instant::now();
    let response = client.execute(request).await?;
    let latency = start.elapsed();

    let status = response.status();
    let resp_headers = response.headers().clone();
    let body = response.bytes().await?;

    log_response(&method, &url, status, latency, None);

    Ok(LoggedResponse {
        status,
        headers: resp_headers,
        body,
        url,
    })
}

fn format_request_body(method: &Method, bytes: &[u8]) -> String {
    if !matches!(method, &Method::POST | &Method::PUT | &Method::PATCH) || bytes.is_empty() {
        return format!("(empty or not applicable for {method})");
    }

    let body_str = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return format_binary_body(bytes),
    };

    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
        redact_json_value(&mut json_value);
        return preview_json(&json_value);
    }

    preview_text(body_str)
}

fn format_response_body(bytes: &[u8], headers: &HeaderMap) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }

    let is_json = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct: &str| ct.contains("application/json"))
        .unwrap_or(false);

    if is_json {
        let body_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return format_binary_body(bytes),
        };

        if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
            redact_json_value(&mut json_value);
            return preview_json(&json_value);
        }
    }

    format_binary_body(bytes)
}

fn redact_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *val = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_json_value(val);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        _ => {}
    }
}

fn preview_json(value: &serde_json::Value) -> String {
    let serialized = value.to_string();
    preview_text(&serialized)
}

fn preview_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_LOGGED_BODY_CHARS {
        return text.to_string();
    }

    let digest = sha2::Sha256::digest(text.as_bytes());
    let hash = format!("{digest:x}")[..12].to_string();
    let preview: String = chars[..MAX_LOGGED_BODY_CHARS].iter().collect();
    format!("{preview}…(truncated,total={},hash={hash})", chars.len())
}

fn format_binary_body(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let hash = format!("{digest:x}")[..12].to_string();
    format!("binary({} bytes,hash={hash})", bytes.len())
}

fn log_request(
    method: &Method,
    url: &reqwest::Url,
    headers: &HeaderMap,
    body_preview: Option<&str>,
) {
    let url = sanitized_url(url);
    let header_fields: Vec<(&str, String)> = headers
        .iter()
        .map(|(name, value)| {
            let value_str = value.to_str().unwrap_or("[binary]").to_string();
            let safe_value = if is_sensitive_header(name.as_str()) {
                "[REDACTED]".to_string()
            } else {
                value_str
            };
            (name.as_str(), safe_value)
        })
        .collect();

    if let Some(body) = body_preview {
        tracing::info!(
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %url,
            http.headers = ?header_fields,
            request_body = body,
            "outgoing request"
        );
    } else {
        tracing::info!(
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %url,
            http.headers = ?header_fields,
            "outgoing request (no body)"
        );
    }
}

fn log_response(
    method: &Method,
    url: &reqwest::Url,
    status: StatusCode,
    latency: std::time::Duration,
    body_preview: Option<&str>,
) {
    let url = sanitized_url(url);
    if let Some(body) = body_preview {
        if status.is_server_error() {
            tracing::event!(
                tracing::Level::ERROR,
                provider = CLIENT_NAME,
                http.method = %method,
                http.url = %url,
                http.status_code = status.as_u16(),
                latency_ms = latency.as_millis(),
                response_body = body,
                "outgoing response"
            );
        } else if status.is_client_error() {
            tracing::event!(
                tracing::Level::WARN,
                provider = CLIENT_NAME,
                http.method = %method,
                http.url = %url,
                http.status_code = status.as_u16(),
                latency_ms = latency.as_millis(),
                response_body = body,
                "outgoing response"
            );
        } else {
            tracing::event!(
                tracing::Level::INFO,
                provider = CLIENT_NAME,
                http.method = %method,
                http.url = %url,
                http.status_code = status.as_u16(),
                latency_ms = latency.as_millis(),
                response_body = body,
                "outgoing response"
            );
        }
    } else if status.is_server_error() {
        tracing::event!(
            tracing::Level::ERROR,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %url,
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        );
    } else if status.is_client_error() {
        tracing::event!(
            tracing::Level::WARN,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %url,
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        );
    } else {
        tracing::event!(
            tracing::Level::INFO,
            provider = CLIENT_NAME,
            http.method = %method,
            http.url = %url,
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        );
    }
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
        let authority = format!("{}:{}", url.host_str().unwrap_or(""), port);
        sanitized = format!("{}://{}{}", url.scheme(), authority, url.path());
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_json_redacts_nested() {
        let mut json = serde_json::json!({
            "athlete_id": "12345",
            "api_key": "secret-key",
            "events": [
                { "name": "test", "token": "abc" }
            ]
        });

        redact_json_value(&mut json);

        assert_eq!(json["api_key"], "[REDACTED]");
        assert_eq!(json["events"][0]["token"], "[REDACTED]");
        assert_eq!(json["athlete_id"], "12345");
    }

    #[test]
    fn preview_text_short() {
        assert_eq!(preview_text("hello"), "hello");
    }

    #[test]
    fn preview_text_truncates() {
        let long = "a".repeat(2000);
        let preview = preview_text(&long);
        assert!(preview.contains("truncated"));
        assert!(preview.contains("total=2000"));
        assert!(!preview.contains(&"a".repeat(1500)));
    }

    #[test]
    fn format_binary_body_formats_binary_payloads() {
        let bytes = b"hello";
        let result = format_binary_body(bytes);
        assert!(result.starts_with("binary(5 bytes,hash="));
    }

    #[test]
    fn sanitized_url_drops_query_string() {
        let url = reqwest::Url::parse("https://intervals.icu/api/v1/athlete/athlete-7/activities?oldest=2026-03-01&newest=2026-03-31").unwrap();
        assert_eq!(
            sanitized_url(&url),
            "https://intervals.icu/api/v1/athlete/athlete-7/activities"
        );
    }
}
