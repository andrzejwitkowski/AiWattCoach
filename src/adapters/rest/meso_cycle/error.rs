use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::domain::meso_cycle::MesoCycleError;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub fn map_meso_cycle_error(error: &MesoCycleError) -> Response {
    let (status, message) = match error {
        MesoCycleError::AlreadyPending => (
            StatusCode::CONFLICT,
            GENERATION_ALREADY_PENDING_MESSAGE.to_string(),
        ),
        MesoCycleError::NotConfigured => (
            StatusCode::BAD_REQUEST,
            "meso cycle llm is not configured".to_string(),
        ),
        MesoCycleError::Validation(message) => (StatusCode::BAD_REQUEST, message.clone()),
        MesoCycleError::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message.clone()),
        MesoCycleError::Repository(message) => (StatusCode::INTERNAL_SERVER_ERROR, message.clone()),
    };

    (status, Json(ErrorBody { error: message })).into_response()
}

const GENERATION_ALREADY_PENDING_MESSAGE: &str = "meso cycle generation is already pending";
