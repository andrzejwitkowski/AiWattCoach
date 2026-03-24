use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::config::AppState;

pub const MAX_MESSAGE_LENGTH: usize = 10_000;
pub const MAX_REQUEST_BODY_BYTES: usize = (MAX_MESSAGE_LENGTH * 6) + 256;

const ACCEPTED_LEVELS: &[&str] = &["info", "warn", "error"];

#[derive(Deserialize)]
pub struct LogIngestionRequest {
    level: String,
    message: String,
    traceparent: Option<String>,
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

    if !ACCEPTED_LEVELS.contains(&body.level.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported_level",
            }),
        )
            .into_response();
    }

    let client_traceparent = body.traceparent.as_deref().unwrap_or("");

    tracing::event!(
        tracing::Level::INFO,
        client_log_level = %body.level,
        client_message = %body.message,
        client_traceparent = %client_traceparent,
        "client log"
    );

    (
        StatusCode::ACCEPTED,
        Json(StatusResponse { status: "accepted" }),
    )
        .into_response()
}
