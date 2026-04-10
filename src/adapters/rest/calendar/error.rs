use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::Level;

use crate::domain::calendar::CalendarError;
use crate::domain::calendar_labels::CalendarLabelError;

use super::super::logging::status_class;

pub(super) fn map_calendar_error(error: CalendarError) -> Response {
    match error {
        CalendarError::NotFound => {
            log_calendar_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        CalendarError::Unauthenticated => {
            log_calendar_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        CalendarError::CredentialsNotConfigured => {
            log_calendar_error(Level::WARN, StatusCode::UNPROCESSABLE_ENTITY, &error);
            StatusCode::UNPROCESSABLE_ENTITY.into_response()
        }
        CalendarError::Validation(_) => {
            log_calendar_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            StatusCode::BAD_REQUEST.into_response()
        }
        CalendarError::Unavailable(_) => {
            log_calendar_error(Level::WARN, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        CalendarError::Internal(_) => {
            log_calendar_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub(super) fn map_calendar_label_error(error: CalendarLabelError) -> Response {
    match error {
        CalendarLabelError::Unauthenticated => {
            log_calendar_label_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        CalendarLabelError::Validation(_) => {
            log_calendar_label_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            StatusCode::BAD_REQUEST.into_response()
        }
        CalendarLabelError::Unavailable(_) => {
            log_calendar_label_error(Level::WARN, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        CalendarLabelError::Internal(_) => {
            log_calendar_label_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_calendar_error(level: Level, status: StatusCode, error: &CalendarError) {
    let error_kind = match error {
        CalendarError::NotFound => "not_found",
        CalendarError::Unauthenticated => "unauthenticated",
        CalendarError::CredentialsNotConfigured => "credentials_not_configured",
        CalendarError::Validation(_) => "validation",
        CalendarError::Unavailable(_) => "unavailable",
        CalendarError::Internal(_) => "internal",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "calendar request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "calendar request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn log_calendar_label_error(level: Level, status: StatusCode, error: &CalendarLabelError) {
    let error_kind = match error {
        CalendarLabelError::Unauthenticated => "unauthenticated",
        CalendarLabelError::Validation(_) => "validation",
        CalendarLabelError::Unavailable(_) => "unavailable",
        CalendarLabelError::Internal(_) => "internal",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "calendar labels request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            error = %error,
            "calendar labels request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
