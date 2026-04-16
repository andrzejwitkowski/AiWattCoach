use sha2::{Digest, Sha256};

const PLANNED_WORKOUT_MARKER_PREFIX: &str = "[AIWATTCOACH:pw=";
const PLANNED_WORKOUT_MARKER_SUFFIX: &str = "]";
const MATCH_TOKEN_LENGTH: usize = 10;

pub fn build_planned_workout_match_token(planned_workout_id: &str) -> String {
    let digest = Sha256::digest(planned_workout_id.as_bytes());
    format!("{digest:x}")
        .chars()
        .take(MATCH_TOKEN_LENGTH)
        .collect::<String>()
        .to_ascii_uppercase()
}

pub fn format_planned_workout_marker(match_token: &str) -> String {
    format!("{PLANNED_WORKOUT_MARKER_PREFIX}{match_token}{PLANNED_WORKOUT_MARKER_SUFFIX}")
}

pub fn extract_planned_workout_marker(text: &str) -> Option<String> {
    let marker_start = text.find(PLANNED_WORKOUT_MARKER_PREFIX)?;
    let token_start = marker_start + PLANNED_WORKOUT_MARKER_PREFIX.len();
    let token_and_suffix = &text[token_start..];
    let token_end = token_and_suffix.find(PLANNED_WORKOUT_MARKER_SUFFIX)?;
    let match_token = token_and_suffix[..token_end].trim();

    if !is_valid_planned_workout_match_token(match_token) {
        return None;
    }

    Some(match_token.to_ascii_uppercase())
}

pub fn append_marker_to_description(description: Option<&str>, marker: &str) -> Option<String> {
    match description.map(str::trim).filter(|value| !value.is_empty()) {
        None => Some(marker.to_string()),
        Some(existing) if existing.contains(marker) => Some(existing.to_string()),
        Some(existing) => Some(format!("{existing}\n\n{marker}")),
    }
}

fn is_valid_planned_workout_match_token(match_token: &str) -> bool {
    match_token.len() == MATCH_TOKEN_LENGTH
        && match_token
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}
