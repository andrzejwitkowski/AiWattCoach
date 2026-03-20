use std::time::Duration;

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::{adapters::mongo::client::verify_connection, config::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: String,
}

#[derive(Serialize)]
pub struct ReadinessResponse {
    status: &'static str,
    reason: Option<&'static str>,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: state.app_name,
    })
}

pub async fn readiness_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<ReadinessResponse>) {
    match verify_connection(
        &state.mongo_client,
        &state.mongo_database,
        Duration::from_secs(2),
    )
    .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "ok",
                reason: None,
            }),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponse {
                status: "degraded",
                reason: Some("mongo_unreachable"),
            }),
        ),
    }
}
