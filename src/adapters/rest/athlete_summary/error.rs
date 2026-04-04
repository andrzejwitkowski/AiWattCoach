use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::domain::athlete_summary::AthleteSummaryError;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub(super) fn map_athlete_summary_error(error: &AthleteSummaryError) -> Response {
    match error {
        AthleteSummaryError::NotConfigured => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
        AthleteSummaryError::Unavailable(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
        AthleteSummaryError::Repository(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "athlete summary service unavailable".to_string(),
            }),
        )
            .into_response(),
        AthleteSummaryError::Llm(error) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}
