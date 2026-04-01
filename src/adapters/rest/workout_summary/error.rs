use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::Level;

use crate::domain::workout_summary::WorkoutSummaryError;

use super::super::logging::status_class;

pub(super) fn map_workout_summary_error(error: &WorkoutSummaryError) -> Response {
    match error {
        WorkoutSummaryError::NotFound => {
            log_workout_summary_error(Level::WARN, StatusCode::NOT_FOUND, error);
            StatusCode::NOT_FOUND.into_response()
        }
        WorkoutSummaryError::Validation(_) => {
            log_workout_summary_error(Level::WARN, StatusCode::BAD_REQUEST, error);
            StatusCode::BAD_REQUEST.into_response()
        }
        WorkoutSummaryError::Locked => {
            log_workout_summary_error(Level::WARN, StatusCode::CONFLICT, error);
            StatusCode::CONFLICT.into_response()
        }
        WorkoutSummaryError::AlreadyExists => {
            log_workout_summary_error(Level::WARN, StatusCode::CONFLICT, error);
            StatusCode::CONFLICT.into_response()
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
