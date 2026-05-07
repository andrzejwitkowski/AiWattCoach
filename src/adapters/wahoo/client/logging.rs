use std::time::Instant;

use bytes::Bytes;
use reqwest::{header::HeaderMap, Method, Request, RequestBuilder, StatusCode};
use sha2::Digest;

use crate::telemetry::is_sensitive_key;

const CLIENT_NAME: &str = "wahoo_oauth";
const CLIENT_LABEL: &str = "wahoo";
const MAX_LOGGED_BODY_CHARS: usize = 1024;
const SAFE_QUERY_KEYS: &[&str] = &["page", "per_page", "sort", "order", "external_id"];

pub struct LoggedResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

#[derive(Clone, Copy, Debug)]
pub enum BodyLoggingMode {
    Full,
}

pub async fn execute_and_log(
    client: &reqwest::Client,
    request: RequestBuilder,
    _body_logging: BodyLoggingMode,
) -> Result<LoggedResponse, reqwest::Error> {
    let request = request.build()?;

    execute_and_log_with_body_request(client, request).await
}

async fn execute_and_log_with_body_request(
    client: &reqwest::Client,
    request: Request,
) -> Result<LoggedResponse, reqwest::Error> {
    let method = request.method().clone();
    let url = request.url().clone();
    let headers = request.headers().clone();
    let body_bytes = request
        .body()
        .and_then(|body| body.as_bytes().map(|bytes| bytes.to_vec()));

    let request_body_preview = body_bytes
        .as_ref()
        .map(|bytes| format_request_body(&method, bytes));

    log_request(&method, &url, &headers, request_body_preview.as_deref());

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

    let response_body_preview = format_response_body(&body, status);

    log_response(&method, &url, status, latency, Some(&response_body_preview));

    Ok(LoggedResponse { status, body })
}

fn format_request_body(method: &Method, bytes: &[u8]) -> String {
    if !matches!(method, &Method::POST | &Method::PUT | &Method::PATCH) || bytes.is_empty() {
        return format!("(empty or not applicable for {method})");
    }

    let body_str = match std::str::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return format_binary_body(bytes),
    };

    if body_str.contains('=') && body_str.contains('&') || body_str.contains("%5B") {
        return preview_form_fields(body_str);
    }

    preview_text(body_str)
}

fn preview_form_fields(body: &str) -> String {
    let redacted: Vec<(String, String)> = body
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = urlencoding::decode(raw_key)
                .map(|value| value.into_owned())
                .unwrap_or_else(|_| raw_key.to_string());
            let value = urlencoding::decode(raw_value)
                .map(|value| value.into_owned())
                .unwrap_or_else(|_| raw_value.to_string());
            let safe_value = if is_sensitive_key(&key) {
                "[REDACTED]".to_string()
            } else {
                value
            };
            (key, safe_value)
        })
        .collect();

    preview_text(&serde_json::to_string(&redacted).unwrap_or_else(|_| "[]".to_string()))
}

fn preview_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_LOGGED_BODY_CHARS {
        return text.to_string();
    }

    let digest = sha2::Sha256::digest(text.as_bytes());
    let hash = format!("{digest:x}");
    let preview: String = chars[..MAX_LOGGED_BODY_CHARS].iter().collect();
    format!(
        "{preview}…(truncated,total={},hash={})",
        chars.len(),
        &hash[..12]
    )
}

fn format_response_body(bytes: &[u8], _status: StatusCode) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }

    let body_str = match std::str::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return format_binary_body(bytes),
    };

    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
        redact_json_value(&mut json_value);
        return preview_text(&json_value.to_string());
    }

    preview_text(body_str)
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

fn format_binary_body(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let hash = format!("{digest:x}");
    format!("binary({} bytes,hash={})", bytes.len(), &hash[..12])
}

fn log_request(
    method: &Method,
    url: &reqwest::Url,
    headers: &HeaderMap,
    body_preview: Option<&str>,
) {
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

    if let Some(body) = body_preview {
        tracing::info!(
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.headers = ?header_fields,
            request_body = body,
            "outgoing request"
        );
    } else {
        tracing::info!(
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
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
    match (response_log_level(status), body_preview) {
        (tracing::Level::ERROR, Some(body)) => tracing::event!(
            tracing::Level::ERROR,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            response_body = body,
            "outgoing response"
        ),
        (tracing::Level::WARN, Some(body)) => tracing::event!(
            tracing::Level::WARN,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            response_body = body,
            "outgoing response"
        ),
        (_, Some(body)) => tracing::event!(
            tracing::Level::INFO,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            response_body = body,
            "outgoing response"
        ),
        (tracing::Level::ERROR, None) => tracing::event!(
            tracing::Level::ERROR,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        ),
        (tracing::Level::WARN, None) => tracing::event!(
            tracing::Level::WARN,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
            http.method = %method,
            http.url = %sanitized_url(url),
            http.status_code = status.as_u16(),
            latency_ms = latency.as_millis(),
            "outgoing response (no body)"
        ),
        _ => tracing::event!(
            tracing::Level::INFO,
            provider = CLIENT_NAME,
            client = CLIENT_LABEL,
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
        client = CLIENT_LABEL,
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

    let safe_query = url
        .query_pairs()
        .filter(|(key, _)| {
            SAFE_QUERY_KEYS
                .iter()
                .any(|allowed| key.eq_ignore_ascii_case(allowed))
        })
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(&key),
                urlencoding::encode(&value)
            )
        })
        .collect::<Vec<_>>();

    if !safe_query.is_empty() {
        sanitized.push('?');
        sanitized.push_str(&safe_query.join("&"));
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::{format_request_body, sanitized_url};
    use reqwest::Method;

    #[test]
    fn format_request_body_redacts_workout_token_in_form_preview() {
        let body = b"workout%5Bname%5D=Threshold&workout%5Bworkout_token%5D=secret-token&workout%5Bminutes%5D=60";

        let preview = format_request_body(&Method::POST, body);

        assert!(preview.contains("workout[name]"));
        assert!(preview.contains("Threshold"));
        assert!(preview.contains("workout[workout_token]"));
        assert!(preview.contains("[REDACTED]"));
        assert!(!preview.contains("secret-token"));
    }

    #[test]
    fn sanitized_url_keeps_safe_wahoo_query_params() {
        let url = reqwest::Url::parse(
            "https://api.wahooligan.com/v1/workouts?page=2&per_page=30&sort=updated_at&order=desc&access_token=secret",
        )
        .unwrap();

        assert_eq!(
            sanitized_url(&url),
            "https://api.wahooligan.com/v1/workouts?page=2&per_page=30&sort=updated_at&order=desc"
        );
    }
}
