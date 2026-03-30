use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    config::AppState,
    domain::{
        identity::IdentityUseCases,
        intervals::IntervalsConnectionTester,
        settings::{UserSettings, UserSettingsUseCases},
    },
};

use super::{
    dto::{
        test_connection_response, TestIntervalsConnectionRequest, UpdateAiAgentsRequest,
        UpdateCyclingRequest, UpdateIntervalsRequest, UpdateOptionsRequest,
    },
    error::{
        map_admin_identity_error, map_connection_error_to_response, map_settings_error,
    },
    intervals_connection::merge_connection_credentials,
    mapping::{
        map_ai_agents_update, map_cycling_update, map_intervals_update, map_options_update,
        map_settings_to_dto,
    },
};

pub async fn get_settings(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    match settings_service.get_settings(&user_id).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn update_ai_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateAiAgentsRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let config = map_ai_agents_update(body, &current);

    match settings_service.update_ai_agents(&user_id, config).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn update_intervals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateIntervalsRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let config = map_intervals_update(body, &current);

    match settings_service.update_intervals(&user_id, config).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn update_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateOptionsRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let options = map_options_update(body, &current);

    match settings_service.update_options(&user_id, options).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn update_cycling(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateCyclingRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let cycling = match map_cycling_update(body, &current) {
        Ok(cycling) => cycling,
        Err(err) => return map_settings_error(&err),
    };

    match settings_service.update_cycling(&user_id, cycling).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn admin_get_user_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Response {
    let identity_service = match identity_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let session_id = match super::super::cookies::read_cookie(&headers, &state.session_cookie_name)
    {
        Some(session_id) => session_id,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => {}
        Err(err) => return map_admin_identity_error(&err),
    }

    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };

    match settings_service.get_settings(&user_id).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn test_intervals_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TestIntervalsConnectionRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Ok(service) => service,
        Err(response) => return response,
    };
    let connection_tester = match connection_tester(&state) {
        Ok(tester) => tester,
        Err(response) => return response,
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let credentials = match merge_connection_credentials(body, &current) {
        Ok(credentials) => credentials,
        Err(response_body) => return Json(response_body).into_response(),
    };

    match connection_tester
        .test_connection(&credentials.api_key, &credentials.athlete_id)
        .await
    {
        Ok(_) => Json(test_connection_response(
            true,
            "Connection successful.",
            credentials.used_saved_api_key,
            credentials.used_saved_athlete_id,
        ))
        .into_response(),
        Err(err) => map_connection_error_to_response(
            err,
            credentials.used_saved_api_key,
            credentials.used_saved_athlete_id,
        ),
    }
}

fn settings_service(state: &AppState) -> Result<&Arc<dyn UserSettingsUseCases>, Response> {
    state
        .settings_service
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

fn identity_service(state: &AppState) -> Result<&Arc<dyn IdentityUseCases>, Response> {
    state
        .identity_service
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

fn connection_tester(
    state: &AppState,
) -> Result<&Arc<dyn IntervalsConnectionTester>, Response> {
    state
        .intervals_connection_tester
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

async fn load_settings(
    settings_service: &Arc<dyn UserSettingsUseCases>,
    user_id: &str,
) -> Result<UserSettings, Response> {
    settings_service
        .get_settings(user_id)
        .await
        .map_err(|err| map_settings_error(&err))
}
