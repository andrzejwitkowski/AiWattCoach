use axum::{extract::State, http::StatusCode, Json};
use mongodb::bson::doc;
use serde::Serialize;

use crate::config::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: String,
    mongo_configured: bool,
}

#[derive(Serialize)]
pub struct ReadinessResponse {
    status: &'static str,
    reason: Option<&'static str>,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: state.settings.app_name,
        mongo_configured: state.mongo_client.is_some(),
    })
}

pub async fn readiness_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<ReadinessResponse>) {
    match &state.mongo_client {
        Some(client) => match client.database("admin").run_command(doc! { "ping": 1 }).await {
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
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponse {
                status: "degraded",
                reason: Some("mongo_unavailable"),
            }),
        ),
    }
}
