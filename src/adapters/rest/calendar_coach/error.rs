use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::Level;

use crate::domain::coach_conversation::CoachConversationError;

use super::super::logging::status_class;

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

pub(super) fn map_calendar_coach_error(error: &CoachConversationError) -> Response {
    match error {
        CoachConversationError::NotFound => {
            log_calendar_coach_error(Level::WARN, StatusCode::NOT_FOUND, error);
            StatusCode::NOT_FOUND.into_response()
        }
        CoachConversationError::Archived | CoachConversationError::ReplyAlreadyPending => {
            log_calendar_coach_error(Level::WARN, StatusCode::CONFLICT, error);
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    message: error.to_string(),
                }),
            )
                .into_response()
        }
        CoachConversationError::Validation(_) => {
            log_calendar_coach_error(Level::WARN, StatusCode::BAD_REQUEST, error);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    message: error.to_string(),
                }),
            )
                .into_response()
        }
        CoachConversationError::Llm(llm_error) => {
            let status = if matches!(llm_error, crate::domain::llm::LlmError::ContextTooLarge(_)) {
                StatusCode::PAYLOAD_TOO_LARGE
            } else if llm_error.is_retryable() {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::BAD_REQUEST
            };
            let public_message =
                if matches!(llm_error, crate::domain::llm::LlmError::ContextTooLarge(_)) {
                    "Conversation context is too large"
                } else if llm_error.is_retryable() {
                    "Calendar coach is temporarily unavailable"
                } else {
                    "Unable to process calendar coach request"
                };
            let level = if status.is_server_error() {
                Level::ERROR
            } else {
                Level::WARN
            };
            log_calendar_coach_error(level, status, error);
            (
                status,
                Json(ErrorResponse {
                    message: public_message.to_string(),
                }),
            )
                .into_response()
        }
        CoachConversationError::Repository(_) => {
            log_calendar_coach_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, error);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

fn log_calendar_coach_error(level: Level, status: StatusCode, error: &CoachConversationError) {
    let error_kind = match error {
        CoachConversationError::NotFound => "not_found",
        CoachConversationError::Archived => "archived",
        CoachConversationError::ReplyAlreadyPending => "reply_already_pending",
        CoachConversationError::Repository(_) => "repository_error",
        CoachConversationError::Llm(_) => "llm_error",
        CoachConversationError::Validation(_) => "validation_error",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "calendar coach request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "calendar coach request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}
