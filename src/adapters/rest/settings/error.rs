use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use tracing::Level;

use crate::domain::{
    identity::IdentityError, intervals::IntervalsConnectionError, settings::SettingsError,
};

use super::super::logging;
use super::dto::{test_connection_response, validation_message_response};

pub(super) fn map_admin_identity_error(err: &IdentityError) -> Response {
    match err {
        IdentityError::Unauthenticated => {
            log_identity_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
        IdentityError::Forbidden => {
            log_identity_error(Level::WARN, StatusCode::FORBIDDEN, err);
            StatusCode::FORBIDDEN.into_response()
        }
        IdentityError::Repository(_) | IdentityError::External(_) => {
            log_identity_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, err);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        IdentityError::EmailNotVerified => {
            log_identity_error(Level::WARN, StatusCode::FORBIDDEN, err);
            StatusCode::FORBIDDEN.into_response()
        }
        IdentityError::InvalidLoginState => {
            log_identity_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
    }
}

pub(super) fn map_settings_error(err: &SettingsError) -> Response {
    match err {
        SettingsError::Repository(_) => {
            log_settings_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, err);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        SettingsError::Unauthenticated => {
            log_settings_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
        SettingsError::Validation(_) => {
            log_settings_error(Level::WARN, StatusCode::BAD_REQUEST, err);
            (
                StatusCode::BAD_REQUEST,
                Json(validation_message_response(&err.to_string())),
            )
                .into_response()
        }
    }
}

pub(super) fn map_connection_error_to_response(
    error: IntervalsConnectionError,
    used_saved_api_key: bool,
    used_saved_athlete_id: bool,
) -> Response {
    match error {
        IntervalsConnectionError::Unauthenticated => {
            log_connection_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            (
                StatusCode::BAD_REQUEST,
                Json(test_connection_response(
                    false,
                    "Invalid API key or athlete ID. Please check your credentials.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                    false,
                )),
            )
                .into_response()
        }
        IntervalsConnectionError::InvalidConfiguration => {
            log_connection_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            (
                StatusCode::BAD_REQUEST,
                Json(test_connection_response(
                    false,
                    "Invalid configuration. Please check athlete ID.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                    false,
                )),
            )
                .into_response()
        }
        IntervalsConnectionError::Unavailable => {
            log_connection_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, &error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(test_connection_response(
                    false,
                    "Intervals.icu is currently unavailable. Please try again later.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                    false,
                )),
            )
                .into_response()
        }
    }
}

fn log_settings_error(level: Level, status: StatusCode, error: &SettingsError) {
    let error_kind = match error {
        SettingsError::Repository(_) => "repository_error",
        SettingsError::Unauthenticated => "unauthenticated",
        SettingsError::Validation(_) => "validation_error",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "settings request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "settings request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn log_connection_error(level: Level, status: StatusCode, error: &IntervalsConnectionError) {
    let error_kind = match error {
        IntervalsConnectionError::InvalidConfiguration => "invalid_configuration",
        IntervalsConnectionError::Unavailable => "unavailable",
        IntervalsConnectionError::Unauthenticated => "unauthenticated",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "settings intervals connection test failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "settings intervals connection test failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn log_identity_error(level: Level, status: StatusCode, error: &IdentityError) {
    let error_kind = match error {
        IdentityError::Unauthenticated => "unauthenticated",
        IdentityError::Forbidden => "forbidden",
        IdentityError::Repository(_) => "repository_error",
        IdentityError::External(_) => "external_error",
        IdentityError::EmailNotVerified => "email_not_verified",
        IdentityError::InvalidLoginState => "invalid_login_state",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "admin identity request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = logging::status_class(status),
            error_kind,
            "admin identity request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
