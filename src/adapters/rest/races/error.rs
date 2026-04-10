use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::Level;

use crate::domain::races::RaceError;

use super::super::logging::status_class;

pub(super) fn map_race_error(error: RaceError) -> Response {
    match error {
        RaceError::NotFound => {
            log_race_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        RaceError::Unauthenticated => {
            log_race_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        RaceError::Validation(_) => {
            log_race_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            StatusCode::BAD_REQUEST.into_response()
        }
        RaceError::Unavailable(_) => {
            log_race_error(Level::WARN, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        RaceError::Internal(_) => {
            log_race_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_race_error(level: Level, status: StatusCode, error: &RaceError) {
    let error_kind = match error {
        RaceError::NotFound => "not_found",
        RaceError::Unauthenticated => "unauthenticated",
        RaceError::Validation(_) => "validation",
        RaceError::Unavailable(_) => "unavailable",
        RaceError::Internal(_) => "internal",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "race request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "race request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
