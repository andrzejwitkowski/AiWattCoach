use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::Level;

use crate::domain::intervals::IntervalsError;

use super::super::logging::status_class;

pub(super) fn map_intervals_error(error: IntervalsError) -> Response {
    match error {
        IntervalsError::Unauthenticated => {
            log_intervals_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        IntervalsError::CredentialsNotConfigured => (
            {
                log_intervals_error(Level::WARN, StatusCode::UNPROCESSABLE_ENTITY, &error);
                StatusCode::UNPROCESSABLE_ENTITY
            },
            "Intervals.icu credentials not configured",
        )
            .into_response(),
        IntervalsError::NotFound => {
            log_intervals_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        IntervalsError::ApiError(_) | IntervalsError::ConnectionError(_) => {
            log_intervals_error(Level::ERROR, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        IntervalsError::Internal(_) => {
            log_intervals_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_intervals_error(level: Level, status: StatusCode, error: &IntervalsError) {
    let error_kind = match error {
        IntervalsError::CredentialsNotConfigured => "credentials_not_configured",
        IntervalsError::NotFound => "not_found",
        IntervalsError::ApiError(_) => "api_error",
        IntervalsError::ConnectionError(_) => "connection_error",
        IntervalsError::Internal(_) => "internal_error",
        IntervalsError::Unauthenticated => "unauthenticated",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "intervals request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "intervals request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
