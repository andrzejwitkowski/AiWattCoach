use sha2::Digest;

use crate::telemetry::is_sensitive_key;

/// Additional HTTP/header-level sensitive keys beyond the core `redact_if_sensitive` set.
const SENSITIVE_HEADER_KEYS: &[&str] = &[
    "authorization",
    "bearer",
    "cookie",
    "set-cookie",
    "x-api-key",
    "proxy-authorization",
    "www-authenticate",
];

/// Redact a raw JSON value in-place by walking the tree and replacing
/// sensitive leaf values with `"[REDACTED]"`.
pub fn redact_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_sensitive_key(key) {
                    redact_sensitive_child_value(val);
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_value(item);
            }
        }
        _ => {}
    }
}

fn redact_sensitive_child_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(_) => redact_value(value),
        serde_json::Value::Array(items) => {
            for item in items {
                match item {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        redact_sensitive_child_value(item);
                    }
                    _ => {
                        *item = serde_json::Value::String("[REDACTED]".to_string());
                    }
                }
            }
        }
        _ => {
            *value = serde_json::Value::String("[REDACTED]".to_string());
        }
    }
}

/// Check whether a header name is sensitive.
pub fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE_HEADER_KEYS.iter().any(|&k| lower == k) || crate::telemetry::is_sensitive_key(&lower)
}

/// Redact sensitive headers from a header map, returning a copy suitable for logging.
pub fn redact_headers(headers: &axum::http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.to_string();
            let value_str = value.to_str().unwrap_or("[binary]").to_string();
            let safe_value = if is_sensitive_header(&name_str) {
                "[REDACTED]".to_string()
            } else {
                value_str
            };
            (name_str, safe_value)
        })
        .collect()
}

/// Format a body preview for logging: up to `max_chars` characters, with a hash suffix.
pub fn format_body_preview(body: &str, max_chars: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    let total = chars.len();

    if total <= max_chars {
        return body.to_string();
    }

    let digest = sha2::Sha256::digest(body.as_bytes());
    let hash_prefix = format!("{digest:x}");
    let hash = &hash_prefix[..12.min(hash_prefix.len())];

    let preview: String = chars[..max_chars].iter().collect();
    format!("{preview}…(truncated,total={total},hash={hash})")
}

/// Format a binary body preview (e.g. .fit files) as shape/hash only.
pub fn format_binary_body_preview(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let hash_prefix = format!("{digest:x}");
    let hash = &hash_prefix[..12.min(hash_prefix.len())];
    format!("binary({} bytes,hash={hash})", bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_value_nested() {
        let mut value: serde_json::Value = serde_json::json!({
            "user": {
                "name": "alice",
                "token": "abc123"
            },
            "data": [
                {"api_key": "key1"},
                {"api_key": "key2"}
            ]
        });

        redact_value(&mut value);

        assert_eq!(value["user"]["name"], "alice");
        assert_eq!(value["user"]["token"], "[REDACTED]");
        assert_eq!(value["data"][0]["api_key"], "[REDACTED]");
        assert_eq!(value["data"][1]["api_key"], "[REDACTED]");
    }

    #[test]
    fn redact_value_redacts_scalar_items_under_sensitive_array_key() {
        let mut value: serde_json::Value = serde_json::json!({
            "user": ["alice", "bob", {"token": "abc123"}]
        });

        redact_value(&mut value);

        assert_eq!(value["user"][0], "[REDACTED]");
        assert_eq!(value["user"][1], "[REDACTED]");
        assert_eq!(value["user"][2]["token"], "[REDACTED]");
    }

    #[test]
    fn sensitive_header_detection() {
        assert!(is_sensitive_header("Authorization"));
        assert!(is_sensitive_header("Cookie"));
        assert!(is_sensitive_header("X-Api-Key"));
        assert!(is_sensitive_header("set-cookie"));
        assert!(!is_sensitive_header("Content-Type"));
        assert!(!is_sensitive_header("X-Request-Id"));
    }

    #[test]
    fn header_redaction() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("Authorization", "Bearer secret-token".parse().unwrap());
        headers.insert("X-Request-Id", "req-123".parse().unwrap());

        let result = redact_headers(&headers);

        assert_eq!(
            result.iter().find(|(n, _)| n == "content-type").unwrap().1,
            "application/json"
        );
        assert_eq!(
            result.iter().find(|(n, _)| n == "authorization").unwrap().1,
            "[REDACTED]"
        );
        assert_eq!(
            result.iter().find(|(n, _)| n == "x-request-id").unwrap().1,
            "req-123"
        );
    }

    #[test]
    fn body_preview_short() {
        assert_eq!(format_body_preview("hello", 100), "hello");
    }

    #[test]
    fn body_preview_truncates() {
        let long = "a".repeat(200);
        let preview = format_body_preview(&long, 10);
        assert!(preview.contains("truncated"));
        assert!(preview.contains("total=200"));
        assert!(!preview.contains(&"a".repeat(100)));
    }

    #[test]
    fn binary_body_preview_format() {
        let bytes = b"hello world";
        let preview = format_binary_body_preview(bytes);
        assert!(preview.starts_with("binary(11 bytes,hash="));
    }
}
