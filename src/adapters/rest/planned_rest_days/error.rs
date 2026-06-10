use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::Level;

use crate::domain::planned_rest_days::PlannedRestDayError;

use super::super::logging::status_class;

pub(super) fn map_planned_rest_day_error(error: PlannedRestDayError) -> Response {
    match error {
        PlannedRestDayError::NotFound => {
            log_planned_rest_day_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        PlannedRestDayError::Unauthenticated => {
            log_planned_rest_day_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        PlannedRestDayError::Validation(_) => {
            log_planned_rest_day_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            StatusCode::BAD_REQUEST.into_response()
        }
        PlannedRestDayError::Internal(_) => {
            log_planned_rest_day_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_planned_rest_day_error(level: Level, status: StatusCode, error: &PlannedRestDayError) {
    let error_kind = match error {
        PlannedRestDayError::NotFound => "not_found",
        PlannedRestDayError::Unauthenticated => "unauthenticated",
        PlannedRestDayError::Validation(_) => "validation",
        PlannedRestDayError::Internal(_) => "internal",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "planned rest day request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "planned rest day request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
