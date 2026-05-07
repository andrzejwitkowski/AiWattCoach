use serde::Serialize;

const MAX_LOGGED_BODY_CHARS: usize = 400;

pub(super) fn serialize_logged_body<T: Serialize>(value: &T) -> String {
    match serde_json::to_string(value) {
        Ok(body) => truncate_logged_body(&body),
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
