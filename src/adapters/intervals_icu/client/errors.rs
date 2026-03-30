use reqwest::StatusCode;

use crate::domain::intervals::IntervalsError;

use super::ApiFailure;

pub(super) fn map_connection_error(error: reqwest::Error) -> IntervalsError {
    IntervalsError::ConnectionError(error.to_string())
}

pub(super) async fn map_error_response(response: reqwest::Response) -> ApiFailure {
    let status = response.status();
    let url = response.url().to_string();
    let response_body = response.text().await.ok().and_then(|body| {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(truncate_log_body(trimmed))
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
    }
}

pub(super) fn map_api_error(error: reqwest::Error) -> IntervalsError {
    let message = error.to_string();

    match error.status() {
        Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => {
            IntervalsError::CredentialsNotConfigured
        }
        None if error.is_connect() || error.is_timeout() => {
            IntervalsError::ConnectionError(message)
        }
        _ => IntervalsError::ApiError(message),
    }
}

fn format_status_code(status: StatusCode) -> String {
    match status.canonical_reason() {
        Some(reason) => format!("{} {}", status.as_u16(), reason),
        None => status.as_u16().to_string(),
    }
}

fn truncate_log_body(body: &str) -> String {
    const MAX_LEN: usize = 512;

    if body.chars().count() <= MAX_LEN {
        return body.to_string();
    }

    let truncated: String = body.chars().take(MAX_LEN).collect();
    format!("{truncated}...")
}
