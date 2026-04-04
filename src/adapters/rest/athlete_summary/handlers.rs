use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    config::AppState,
    domain::{athlete_summary::AthleteSummaryUseCases, identity::IdentityUseCases},
};

use super::{error::map_athlete_summary_error, mapping::map_summary_state_to_dto};

pub async fn get_athlete_summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match athlete_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service.get_summary_state(&user_id).await {
        Ok(state) => Json(map_summary_state_to_dto(state)).into_response(),
        Err(error) => map_athlete_summary_error(&error),
    }
}

pub async fn generate_athlete_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match athlete_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service.generate_summary(&user_id, true).await {
        Ok(summary) => Json(map_summary_state_to_dto(
            crate::domain::athlete_summary::AthleteSummaryState {
                summary: Some(summary),
                stale: false,
            },
        ))
        .into_response(),
        Err(error) => map_athlete_summary_error(&error),
    }
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

fn athlete_summary_service(state: &AppState) -> Option<&Arc<dyn AthleteSummaryUseCases>> {
    state.athlete_summary_service.as_ref()
}

#[allow(dead_code)]
fn identity_service(state: &AppState) -> Option<&Arc<dyn IdentityUseCases>> {
    state.identity_service.as_ref()
}
