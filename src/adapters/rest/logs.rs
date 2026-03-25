use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
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
    headers: HeaderMap,
    Json(body): Json<LogIngestionRequest>,
) -> Response {
    if !state.client_log_ingestion_enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "disabled" }),
        )
            .into_response();
    }

    if !request_has_same_origin(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "invalid_origin",
            }),
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

fn request_has_same_origin(headers: &HeaderMap) -> bool {
    let Some(host) = header_value(headers, header::HOST) else {
        return false;
    };
    let Some(origin) = header_value(headers, header::ORIGIN) else {
        return false;
    };

    let Ok(origin_uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };

    if let Some(fetch_site) =
        header_value(headers, header::HeaderName::from_static("sec-fetch-site"))
    {
        if fetch_site != "same-origin" && fetch_site != "same-site" {
            return false;
        }
    }

    matches!(origin_uri.scheme_str(), Some("http" | "https"))
        && origin_uri.authority().map(|authority| authority.as_str()) == Some(host)
}

fn header_value(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}
