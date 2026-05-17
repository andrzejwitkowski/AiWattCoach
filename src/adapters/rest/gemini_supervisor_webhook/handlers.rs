use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use tracing::warn;

use crate::{config::AppState, domain::training_plan_supervisor::GeminiSupervisorWebhookOutcome};

use super::dto::GeminiSupervisorWebhookRequest;

#[derive(Serialize)]
struct GeminiSupervisorWebhookResponse {
    accepted: bool,
}

pub async fn receive_webhook(
    State(state): State<AppState>,
    Path((worker_operation_key, webhook_token)): Path<(String, String)>,
    Json(payload): Json<GeminiSupervisorWebhookRequest>,
) -> impl IntoResponse {
    let Some(service) = state.training_plan_supervisor_webhook_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service
        .receive_gemini_batch_webhook(
            &worker_operation_key,
            &webhook_token,
            &payload.event_type,
            &payload.data.id,
        )
        .await
    {
        Ok(GeminiSupervisorWebhookOutcome::Ignored) => {
            Json(GeminiSupervisorWebhookResponse { accepted: false }).into_response()
        }
        Ok(GeminiSupervisorWebhookOutcome::Accepted(_)) => {
            Json(GeminiSupervisorWebhookResponse { accepted: true }).into_response()
        }
        Err(crate::domain::training_plan::TrainingPlanError::Validation(message))
            if message == "Gemini supervisor webhook token is invalid" =>
        {
            StatusCode::UNAUTHORIZED.into_response()
        }
        Err(crate::domain::training_plan::TrainingPlanError::Unavailable(message))
            if message == "Gemini supervisor webhook is not configured" =>
        {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(crate::domain::training_plan::TrainingPlanError::Validation(_)) => {
            StatusCode::BAD_REQUEST.into_response()
        }
        Err(error) => {
            warn!(
                worker_operation_key,
                batch_name = %payload.data.id,
                error = %error,
                "Gemini supervisor webhook processing failed"
            );
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
