use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::Level;

use crate::domain::workout_summary::WorkoutSummaryError;

use super::super::logging::status_class;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub(super) fn map_workout_summary_error(error: &WorkoutSummaryError) -> Response {
    match error {
        WorkoutSummaryError::NotFound => {
            log_workout_summary_error(Level::WARN, StatusCode::NOT_FOUND, error);
            StatusCode::NOT_FOUND.into_response()
        }
        WorkoutSummaryError::Validation(_) => {
            log_workout_summary_error(Level::WARN, StatusCode::BAD_REQUEST, error);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
        WorkoutSummaryError::Locked => {
            log_workout_summary_error(Level::WARN, StatusCode::CONFLICT, error);
            StatusCode::CONFLICT.into_response()
        }
        WorkoutSummaryError::ReplyAlreadyPending => {
            log_workout_summary_error(Level::WARN, StatusCode::CONFLICT, error);
            StatusCode::CONFLICT.into_response()
        }
        WorkoutSummaryError::AlreadyExists => {
            log_workout_summary_error(Level::WARN, StatusCode::CONFLICT, error);
            StatusCode::CONFLICT.into_response()
        }
        WorkoutSummaryError::Llm(llm_error) => {
            let status = if matches!(llm_error, crate::domain::llm::LlmError::ContextTooLarge(_)) {
                StatusCode::PAYLOAD_TOO_LARGE
            } else if llm_error.is_retryable() {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::BAD_REQUEST
            };
            let level = if status.is_server_error() {
                Level::ERROR
            } else {
                Level::WARN
            };
            log_workout_summary_error(level, status, error);
            status.into_response()
        }
        WorkoutSummaryError::Repository(_) => {
            log_workout_summary_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, error);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

fn log_workout_summary_error(level: Level, status: StatusCode, error: &WorkoutSummaryError) {
    let error_kind = match error {
        WorkoutSummaryError::AlreadyExists => "already_exists",
        WorkoutSummaryError::Locked => "locked",
        WorkoutSummaryError::ReplyAlreadyPending => "reply_already_pending",
        WorkoutSummaryError::Llm(_) => "llm_error",
        WorkoutSummaryError::NotFound => "not_found",
        WorkoutSummaryError::Repository(_) => "repository_error",
        WorkoutSummaryError::Validation(_) => "validation_error",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "workout summary request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "workout summary request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
