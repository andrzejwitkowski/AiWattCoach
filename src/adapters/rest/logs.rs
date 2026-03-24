use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::Level;

use crate::config::AppState;

pub const MAX_MESSAGE_LENGTH: usize = 10_000;
pub const MAX_REQUEST_BODY_BYTES: usize = (MAX_MESSAGE_LENGTH * 6) + 256;

#[derive(Deserialize)]
pub struct LogIngestionRequest {
    level: String,
    message: String,
}

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub async fn ingest_logs(
    State(state): State<AppState>,
    Json(body): Json<LogIngestionRequest>,
) -> Response {
    if !state.client_log_ingestion_enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "disabled" }),
        )
            .into_response();
    }

    if body.message.len() > MAX_MESSAGE_LENGTH {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "message_too_long",
            }),
        )
            .into_response();
    }

    let level = match parse_level(&body.level) {
        Some(level) => level,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "unsupported_level",
                }),
            )
                .into_response()
        }
    };

    match level {
        Level::INFO => tracing::event!(
            Level::INFO,
            client_log_level = %body.level,
            client_message = %body.message,
            "client log"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            client_log_level = %body.level,
            client_message = %body.message,
            "client log"
        ),
        Level::ERROR => tracing::event!(
            Level::ERROR,
            client_log_level = %body.level,
            client_message = %body.message,
            "client log"
        ),
        _ => unreachable!("parse_level only returns accepted levels"),
    }

    (
        StatusCode::ACCEPTED,
        Json(StatusResponse { status: "accepted" }),
    )
        .into_response()
}

fn parse_level(level: &str) -> Option<Level> {
    match level {
        "info" => Some(Level::INFO),
        "warn" => Some(Level::WARN),
        "error" => Some(Level::ERROR),
        _ => None,
    }
}
