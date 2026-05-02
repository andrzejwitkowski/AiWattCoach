use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::Value;
use tracing::warn;

use crate::{
    config::AppState,
    domain::wahoo::{WahooWebhookError, WahooWebhookOutcome},
};

use super::dto::{WahooWebhookDomainParts, WahooWebhookRequest};

#[derive(Serialize)]
struct WahooWebhookResponse {
    accepted: bool,
}

pub async fn receive_webhook(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let Some(service) = state.wahoo_webhook_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    if payload
        .get("event_type")
        .and_then(Value::as_str)
        .is_some_and(|event_type| event_type != "workout_summary")
    {
        return Json(WahooWebhookResponse { accepted: false }).into_response();
    }

    let payload = match serde_json::from_value::<WahooWebhookRequest>(payload) {
        Ok(payload) => payload,
        Err(error) => {
            warn!(error = %error, "Invalid Wahoo webhook payload");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let webhook_token = payload.webhook_token.clone();
    let (_, parts) = match payload.into_domain_parts() {
        Ok(parts) => parts,
        Err(error) => {
            warn!(error, "Invalid Wahoo webhook payload");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let WahooWebhookDomainParts {
        wahoo_user_id,
        workout,
    } = parts;

    match service
        .import_webhook_workout(&webhook_token, wahoo_user_id, workout)
        .await
    {
        Ok(WahooWebhookOutcome::Ignored) => {
            Json(WahooWebhookResponse { accepted: false }).into_response()
        }
        Ok(WahooWebhookOutcome::Accepted(_)) => {
            Json(WahooWebhookResponse { accepted: true }).into_response()
        }
        Err(WahooWebhookError::Unauthorized) => StatusCode::UNAUTHORIZED.into_response(),
        Err(WahooWebhookError::InvalidPayload(_)) => StatusCode::BAD_REQUEST.into_response(),
        Err(WahooWebhookError::NotConfigured) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(error) => {
            warn!(wahoo_user_id, error = %error, "Wahoo webhook import failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
