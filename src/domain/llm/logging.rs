use std::sync::OnceLock;

use serde::Serialize;

use crate::telemetry::is_sensitive_key;

const DEFAULT_MAX_LOGGED_BODY_CHARS: usize = 400;
const FULL_DEBUG_MAX_LOGGED_BODY_CHARS: usize = 20_000;

pub fn serialize_logged_body<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(mut body) => {
            redact_logged_json(&mut body);
            truncate_logged_body(&body.to_string())
        }
        Err(error) => format!("(body serialization failed: {error})"),
    }
}

pub fn truncate_logged_body(body: &str) -> String {
    let max_logged_body_chars = max_logged_body_chars();
    if let Some((cutoff, _)) = body.char_indices().nth(max_logged_body_chars) {
        return format!("{}...(truncated)", &body[..cutoff]);
    }

    body.to_string()
}

pub fn llm_full_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var("ENABLE_LLM_FULL_DEBUG_LOGGING")
            .ok()
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn max_logged_body_chars() -> usize {
    if llm_full_debug_logging_enabled() {
        FULL_DEBUG_MAX_LOGGED_BODY_CHARS
    } else {
        DEFAULT_MAX_LOGGED_BODY_CHARS
    }
}

fn redact_logged_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *child = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_logged_json(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_logged_json(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::{serialize_logged_body, truncate_logged_body};

    #[derive(Serialize)]
    struct LoggedPayload {
        api_key: String,
        nested: NestedPayload,
    }

    #[derive(Serialize)]
    struct NestedPayload {
        access_token: String,
        label: String,
    }

    #[test]
    fn truncates_logged_llm_bodies_at_default_limit() {
        let body = "x".repeat(425);

        let truncated = truncate_logged_body(&body);

        assert_eq!(truncated.len(), 400 + "...(truncated)".len());
        assert!(truncated.ends_with("...(truncated)"));
    }

    #[test]
    fn serialize_logged_body_redacts_sensitive_fields() {
        let body = serialize_logged_body(&LoggedPayload {
            api_key: "secret-key".to_string(),
            nested: NestedPayload {
                access_token: "token-123".to_string(),
                label: "safe".to_string(),
            },
        });

        assert!(body.contains("[REDACTED]"));
        assert!(!body.contains("secret-key"));
        assert!(!body.contains("token-123"));
        assert!(body.contains("safe"));
    }
}
