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
        llm::LlmChatPort,
        settings::{UserSettings, UserSettingsUseCases},
    },
};

use super::{
    ai_connection::{
        build_test_request, map_ai_connection_error_to_response, merge_ai_connection_config,
    },
    dto::{
        test_connection_response, TestIntervalsConnectionRequest, UpdateAiAgentsRequest,
        UpdateAvailabilityRequest, UpdateCyclingRequest, UpdateIntervalsRequest,
        UpdateOptionsRequest,
    },
    error::{map_admin_identity_error, map_connection_error_to_response, map_settings_error},
    intervals_connection::{
        build_persisted_intervals_config, can_persist_tested_credentials,
        merge_connection_credentials, should_persist_tested_credentials,
    },
    mapping::{
        map_ai_agents_update, map_availability_update, map_cycling_update, map_intervals_update,
        map_options_update, map_settings_to_dto,
    },
};

pub async fn get_settings(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let config = match map_ai_agents_update(body, &current) {
        Ok(config) => config,
        Err(err) => return map_settings_error(&err),
    };

    match settings_service.update_ai_agents(&user_id, config).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn test_ai_agents_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateAiAgentsRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let llm_chat_service = match llm_chat_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let current = match load_settings(settings_service, &user_id).await {
        Ok(settings) => settings,
        Err(response) => return response,
    };
    let merged = match merge_ai_connection_config(body, &current) {
        Ok(config) => config,
        Err(response_body) => {
            return (StatusCode::BAD_REQUEST, Json(response_body)).into_response()
        }
    };

    match llm_chat_service
        .chat(merged.config, build_test_request(&user_id))
        .await
    {
        Ok(_) => Json(super::dto::test_ai_agents_connection_response(
            true,
            "Connection successful.",
            merged.used_saved_api_key,
            merged.used_saved_provider,
            merged.used_saved_model,
        ))
        .into_response(),
        Err(error) => {
            let (status, body) = map_ai_connection_error_to_response(
                error,
                merged.used_saved_api_key,
                merged.used_saved_provider,
                merged.used_saved_model,
            );
            (status, body).into_response()
        }
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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

pub async fn update_availability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateAvailabilityRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let settings_service = match settings_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let availability = match map_availability_update(body) {
        Ok(availability) => availability,
        Err(err) => return map_settings_error(&err),
    };

    match settings_service
        .update_availability(&user_id, availability)
        .await
    {
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let connection_tester = match connection_tester(&state) {
        Some(tester) => tester,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
        Ok(_) => {
            let latest_current = match load_settings(settings_service, &user_id).await {
                Ok(settings) => settings,
                Err(response) => return response,
            };
            let persisted_status_updated =
                if should_persist_tested_credentials(&credentials, &latest_current)
                    && can_persist_tested_credentials(&current, &latest_current)
                {
                    let config = build_persisted_intervals_config(&credentials);
                    match settings_service.update_intervals(&user_id, config).await {
                        Ok(_) => true,
                        Err(err) => return map_settings_error(&err),
                    }
                } else {
                    false
                };

            Json(test_connection_response(
                true,
                "Connection successful.",
                credentials.used_saved_api_key,
                credentials.used_saved_athlete_id,
                persisted_status_updated,
            ))
            .into_response()
        }
        Err(err) => map_connection_error_to_response(
            err,
            credentials.used_saved_api_key,
            credentials.used_saved_athlete_id,
        ),
    }
}

fn settings_service(state: &AppState) -> Option<&Arc<dyn UserSettingsUseCases>> {
    state.settings_service.as_ref()
}

fn identity_service(state: &AppState) -> Option<&Arc<dyn IdentityUseCases>> {
    state.identity_service.as_ref()
}

fn connection_tester(state: &AppState) -> Option<&Arc<dyn IntervalsConnectionTester>> {
    state.intervals_connection_tester.as_ref()
}

fn llm_chat_service(state: &AppState) -> Option<&Arc<dyn LlmChatPort>> {
    state.llm_chat_service.as_ref()
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
