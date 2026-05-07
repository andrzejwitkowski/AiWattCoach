use reqwest::StatusCode;
use sha2::Digest;

use crate::telemetry::is_sensitive_key;

const PROVIDER: &str = "google_oauth";
const MAX_LOGGED_BODY_CHARS: usize = 1024;

pub(super) fn log_request(method: &str, url: &str, body: &[(String, String)]) {
    let preview = preview_form_body(body);
    tracing::info!(
        provider = PROVIDER,
        http.method = method,
        http.url = url,
        request_body = preview,
        "outgoing request"
    );
}

pub(super) fn log_response(method: &str, url: &str, status: StatusCode, body: &str) {
    let response_body_bytes = body.len();
    let response_body_hash = body_hash(body.as_bytes());

    if status.is_server_error() {
        tracing::error!(
            provider = PROVIDER,
            http.method = method,
            http.url = url,
            http.status_code = status.as_u16(),
            response_body_bytes,
            response_body_hash,
            "outgoing response"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            provider = PROVIDER,
            http.method = method,
            http.url = url,
            http.status_code = status.as_u16(),
            response_body_bytes,
            response_body_hash,
            "outgoing response"
        );
    } else {
        tracing::info!(
            provider = PROVIDER,
            http.method = method,
            http.url = url,
            http.status_code = status.as_u16(),
            response_body_bytes,
            response_body_hash,
            "outgoing response"
        );
    }
}

pub(super) fn preview_form_body(body: &[(String, String)]) -> String {
    let redacted = body
        .iter()
        .map(|(key, value)| {
            let safe_value = if is_sensitive_oauth_form_key(key) {
                "[REDACTED]".to_string()
            } else {
                value.clone()
            };
            (key.clone(), safe_value)
        })
        .collect::<Vec<_>>();

    match serde_json::to_string(&redacted) {
        Ok(value) => truncate_logged_body(&value),
        Err(error) => format!("(body serialization failed: {error})"),
    }
}

pub(super) fn truncate_logged_body(body: &str) -> String {
    if body.chars().count() <= MAX_LOGGED_BODY_CHARS {
        return body.to_string();
    }

    let truncated: String = body.chars().take(MAX_LOGGED_BODY_CHARS).collect();
    format!("{truncated}...(truncated)")
}

fn is_sensitive_oauth_form_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("code") || is_sensitive_key(key)
}

fn body_hash(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    format!("{digest:x}")[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::{body_hash, preview_form_body};

    #[test]
    fn preview_form_body_redacts_oauth_code() {
        let preview = preview_form_body(&[
            ("code".to_string(), "oauth-code-123".to_string()),
            ("grant_type".to_string(), "authorization_code".to_string()),
        ]);

        assert!(preview.contains("[REDACTED]"));
        assert!(!preview.contains("oauth-code-123"));
    }

    #[test]
    fn body_hash_returns_short_sha_preview() {
        let hash = body_hash(br#"{"access_token":"secret"}"#);

        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
    }
}
