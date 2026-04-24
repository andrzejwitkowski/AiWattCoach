use reqwest::StatusCode;
use serde_json::Value;
use sha2::Digest;

use crate::domain::intervals::IntervalsError;

use super::{logging::LoggedResponse, ApiFailure};

const STRAVA_ACTIVITY_API_UNAVAILABLE_ERROR: &str = "Cannot read Strava activities via the API";

pub(super) fn map_connection_error(error: reqwest::Error) -> IntervalsError {
    IntervalsError::ConnectionError(error.to_string())
}

pub(super) fn map_error_response_from_logged_response(response: LoggedResponse) -> ApiFailure {
    let status = response.status;
    let url = sanitize_error_url(&response.url);
    let response_body_utf8 = std::str::from_utf8(&response.body).ok();
    let known_strava_activity_details_unavailable = response_body_utf8
        .is_some_and(|body| is_known_strava_activity_details_unavailable(status, body));
    let response_body = response_body_utf8.and_then(|body| {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(summarize_log_body(trimmed.as_bytes()))
        }
    });

    let error = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            IntervalsError::CredentialsNotConfigured
        }
        _ => {
            let mut message = format!("HTTP {} for url ({url})", format_status_code(status));
            if let Some(body) = response_body.as_deref() {
                message.push_str("; response body: ");
                message.push_str(body);
            }
            IntervalsError::ApiError(message)
        }
    };

    ApiFailure {
        status: Some(status),
        error,
        response_body,
        known_strava_activity_details_unavailable,
    }
}

fn is_known_strava_activity_details_unavailable(status: StatusCode, body: &str) -> bool {
    if status != StatusCode::UNPROCESSABLE_ENTITY {
        return false;
    }

    serde_json::from_str::<Value>(body)
        .ok()
        .is_some_and(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|error| error == STRAVA_ACTIVITY_API_UNAVAILABLE_ERROR)
        })
}

fn sanitize_error_url(url: &reqwest::Url) -> String {
    let mut sanitized = url.clone();
    sanitized.set_query(None);
    sanitized.set_fragment(None);
    sanitized.to_string()
}

fn format_status_code(status: StatusCode) -> String {
    match status.canonical_reason() {
        Some(reason) => format!("{} {}", status.as_u16(), reason),
        None => status.as_u16().to_string(),
    }
}

pub(super) fn summarize_log_body(body: &[u8]) -> String {
    let digest = sha2::Sha256::digest(body);
    let hash = format!("{digest:x}");
    format!(
        "payload bytes={} hash={}",
        body.len(),
        &hash[..12.min(hash.len())]
    )
}
