use std::sync::OnceLock;

use serde::Serialize;

const DEFAULT_MAX_LOGGED_BODY_CHARS: usize = 400;
const FULL_DEBUG_MAX_LOGGED_BODY_CHARS: usize = 20_000;

pub fn serialize_logged_body<T: Serialize>(value: &T) -> String {
    match serde_json::to_string(value) {
        Ok(body) => truncate_logged_body(&body),
        Err(error) => format!("(body serialization failed: {error})"),
    }
}

pub fn truncate_logged_body(body: &str) -> String {
    let max_logged_body_chars = max_logged_body_chars();
    if body.chars().count() <= max_logged_body_chars {
        return body.to_string();
    }

    let truncated: String = body.chars().take(max_logged_body_chars).collect();
    format!("{truncated}...(truncated)")
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

#[cfg(test)]
mod tests {
    use super::truncate_logged_body;

    #[test]
    fn truncates_logged_llm_bodies_at_default_limit() {
        let body = "x".repeat(425);

        let truncated = truncate_logged_body(&body);

        assert_eq!(truncated.len(), 400 + "...(truncated)".len());
        assert!(truncated.ends_with("...(truncated)"));
    }
}
