use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::Level;

use crate::{
    config::AppState,
    domain::identity::IdentityError,
    domain::intervals::IntervalsConnectionError,
    domain::settings::{
        mask_sensitive, validation, AiAgentsConfig, AnalysisOptions, CyclingSettings,
        IntervalsConfig, SettingsError,
    },
};

use super::cookies::read_cookie;
use super::logging::status_class;

#[derive(Serialize)]
pub struct UserSettingsDto {
    #[serde(rename = "aiAgents")]
    ai_agents: AiAgentsDto,
    intervals: IntervalsDto,
    options: OptionsDto,
    cycling: CyclingDto,
}

#[derive(Serialize)]
struct AiAgentsDto {
    #[serde(rename = "openaiApiKey")]
    openai_api_key: Option<String>,
    #[serde(rename = "openaiApiKeySet")]
    openai_api_key_set: bool,
    #[serde(rename = "geminiApiKey")]
    gemini_api_key: Option<String>,
    #[serde(rename = "geminiApiKeySet")]
    gemini_api_key_set: bool,
}

#[derive(Serialize)]
struct IntervalsDto {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    #[serde(rename = "apiKeySet")]
    api_key_set: bool,
    #[serde(rename = "athleteId")]
    athlete_id: Option<String>,
    connected: bool,
}

#[derive(Serialize)]
struct OptionsDto {
    #[serde(rename = "analyzeWithoutHeartRate")]
    analyze_without_heart_rate: bool,
}

#[derive(Serialize)]
struct CyclingDto {
    #[serde(rename = "fullName")]
    full_name: Option<String>,
    age: Option<u32>,
    #[serde(rename = "heightCm")]
    height_cm: Option<u32>,
    #[serde(rename = "weightKg")]
    weight_kg: Option<f64>,
    #[serde(rename = "ftpWatts")]
    ftp_watts: Option<u32>,
    #[serde(rename = "hrMaxBpm")]
    hr_max_bpm: Option<u32>,
    #[serde(rename = "vo2Max")]
    vo2_max: Option<f64>,
    #[serde(rename = "lastZoneUpdateEpochSeconds")]
    last_zone_update_epoch_seconds: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateAiAgentsRequest {
    #[serde(rename = "openaiApiKey")]
    openai_api_key: Option<String>,
    #[serde(rename = "geminiApiKey")]
    gemini_api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateIntervalsRequest {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    #[serde(rename = "athleteId")]
    athlete_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateOptionsRequest {
    #[serde(rename = "analyzeWithoutHeartRate")]
    analyze_without_heart_rate: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateCyclingRequest {
    #[serde(rename = "fullName")]
    full_name: Option<String>,
    age: Option<u32>,
    #[serde(rename = "heightCm")]
    height_cm: Option<u32>,
    #[serde(rename = "weightKg")]
    weight_kg: Option<f64>,
    #[serde(rename = "ftpWatts")]
    ftp_watts: Option<u32>,
    #[serde(rename = "hrMaxBpm")]
    hr_max_bpm: Option<u32>,
    #[serde(rename = "vo2Max")]
    vo2_max: Option<f64>,
}

pub async fn get_settings(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
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
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let current = match settings_service.get_settings(&user_id).await {
        Ok(s) => s,
        Err(err) => return map_settings_error(&err),
    };

    let config = AiAgentsConfig {
        openai_api_key: body.openai_api_key.or(current.ai_agents.openai_api_key),
        gemini_api_key: body.gemini_api_key.or(current.ai_agents.gemini_api_key),
    };

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
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let current = match settings_service.get_settings(&user_id).await {
        Ok(s) => s,
        Err(err) => return map_settings_error(&err),
    };

    let config = IntervalsConfig {
        api_key: body.api_key.or(current.intervals.api_key),
        athlete_id: body.athlete_id.or(current.intervals.athlete_id),
        connected: current.intervals.connected,
    };

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
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let current = match settings_service.get_settings(&user_id).await {
        Ok(s) => s,
        Err(err) => return map_settings_error(&err),
    };

    let options = AnalysisOptions {
        analyze_without_heart_rate: body
            .analyze_without_heart_rate
            .unwrap_or(current.options.analyze_without_heart_rate),
    };

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
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let current = match settings_service.get_settings(&user_id).await {
        Ok(s) => s,
        Err(err) => return map_settings_error(&err),
    };

    let age = match validation::validate_cycling_age(body.age.or(current.cycling.age)) {
        Ok(v) => v,
        Err(e) => return map_settings_error(&e),
    };
    let height_cm =
        match validation::validate_cycling_height(body.height_cm.or(current.cycling.height_cm)) {
            Ok(v) => v,
            Err(e) => return map_settings_error(&e),
        };
    let weight_kg =
        match validation::validate_cycling_weight(body.weight_kg.or(current.cycling.weight_kg)) {
            Ok(v) => v,
            Err(e) => return map_settings_error(&e),
        };
    let ftp_watts =
        match validation::validate_cycling_ftp(body.ftp_watts.or(current.cycling.ftp_watts)) {
            Ok(v) => v,
            Err(e) => return map_settings_error(&e),
        };
    let hr_max_bpm =
        match validation::validate_cycling_hr(body.hr_max_bpm.or(current.cycling.hr_max_bpm)) {
            Ok(v) => v,
            Err(e) => return map_settings_error(&e),
        };
    let vo2_max = match validation::validate_cycling_vo2(body.vo2_max.or(current.cycling.vo2_max)) {
        Ok(v) => v,
        Err(e) => return map_settings_error(&e),
    };

    let cycling = CyclingSettings {
        full_name: body.full_name.or(current.cycling.full_name),
        age,
        height_cm,
        weight_kg,
        ftp_watts,
        hr_max_bpm,
        vo2_max,
        last_zone_update_epoch_seconds: current.cycling.last_zone_update_epoch_seconds,
    };

    match settings_service.update_cycling(&user_id, cycling).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

pub async fn admin_get_user_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Response {
    let identity_service = match state.identity_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let session_id = match read_cookie(&headers, &state.session_cookie_name) {
        Some(id) => id,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => {}
        Err(err) => return map_admin_identity_error(&err),
    }

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match settings_service.get_settings(&user_id).await {
        Ok(settings) => Json(map_settings_to_dto(&settings)).into_response(),
        Err(err) => map_settings_error(&err),
    }
}

fn map_admin_identity_error(err: &IdentityError) -> Response {
    match err {
        IdentityError::Unauthenticated => {
            log_identity_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
        IdentityError::Forbidden => {
            log_identity_error(Level::WARN, StatusCode::FORBIDDEN, err);
            StatusCode::FORBIDDEN.into_response()
        }
        IdentityError::Repository(_)
        | IdentityError::External(_) => {
            log_identity_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, err);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        IdentityError::EmailNotVerified => {
            log_identity_error(Level::WARN, StatusCode::FORBIDDEN, err);
            StatusCode::FORBIDDEN.into_response()
        }
        IdentityError::InvalidLoginState => {
            log_identity_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
    }
}

fn map_settings_error(err: &SettingsError) -> Response {
    match err {
        SettingsError::Repository(_) => {
            log_settings_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, err);
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        SettingsError::Unauthenticated => {
            log_settings_error(Level::WARN, StatusCode::UNAUTHORIZED, err);
            StatusCode::UNAUTHORIZED.into_response()
        }
        SettingsError::Validation(_) => {
            log_settings_error(Level::WARN, StatusCode::BAD_REQUEST, err);
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

fn map_settings_to_dto(settings: &crate::domain::settings::UserSettings) -> UserSettingsDto {
    UserSettingsDto {
        ai_agents: AiAgentsDto {
            openai_api_key: mask_sensitive(&settings.ai_agents.openai_api_key),
            openai_api_key_set: settings.ai_agents.openai_api_key.is_some(),
            gemini_api_key: mask_sensitive(&settings.ai_agents.gemini_api_key),
            gemini_api_key_set: settings.ai_agents.gemini_api_key.is_some(),
        },
        intervals: IntervalsDto {
            api_key: mask_sensitive(&settings.intervals.api_key),
            api_key_set: settings.intervals.api_key.is_some(),
            athlete_id: settings.intervals.athlete_id.clone(),
            connected: settings.intervals.connected,
        },
        options: OptionsDto {
            analyze_without_heart_rate: settings.options.analyze_without_heart_rate,
        },
        cycling: CyclingDto {
            full_name: settings.cycling.full_name.clone(),
            age: settings.cycling.age,
            height_cm: settings.cycling.height_cm,
            weight_kg: settings.cycling.weight_kg,
            ftp_watts: settings.cycling.ftp_watts,
            hr_max_bpm: settings.cycling.hr_max_bpm,
            vo2_max: settings.cycling.vo2_max,
            last_zone_update_epoch_seconds: settings.cycling.last_zone_update_epoch_seconds,
        },
    }
}

#[derive(Deserialize)]
pub struct TestIntervalsConnectionRequest {
    #[serde(rename = "apiKey")]
    api_key: Option<String>,
    #[serde(rename = "athleteId")]
    athlete_id: Option<String>,
}

#[derive(Serialize)]
struct TestIntervalsConnectionResponse {
    connected: bool,
    message: String,
    #[serde(rename = "usedSavedApiKey")]
    used_saved_api_key: bool,
    #[serde(rename = "usedSavedAthleteId")]
    used_saved_athlete_id: bool,
    #[serde(rename = "persistedStatusUpdated")]
    persisted_status_updated: bool,
}

fn normalize_optional_input(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

struct MergedCredentials {
    api_key: String,
    athlete_id: String,
    used_saved_api_key: bool,
    used_saved_athlete_id: bool,
}

fn merge_credentials(
    transient_api_key: Option<String>,
    transient_athlete_id: Option<String>,
    saved_api_key: Option<String>,
    saved_athlete_id: Option<String>,
) -> Option<MergedCredentials> {
    let effective_api_key = transient_api_key.clone().or_else(|| saved_api_key.clone());
    let effective_athlete_id = transient_athlete_id
        .clone()
        .or_else(|| saved_athlete_id.clone());

    let used_saved_api_key = transient_api_key.is_none() && saved_api_key.is_some();
    let used_saved_athlete_id = transient_athlete_id.is_none() && saved_athlete_id.is_some();

    match (effective_api_key, effective_athlete_id) {
        (Some(api_key), Some(athlete_id)) => Some(MergedCredentials {
            api_key,
            athlete_id,
            used_saved_api_key,
            used_saved_athlete_id,
        }),
        _ => None,
    }
}

fn test_connection_response(
    connected: bool,
    message: &str,
    used_saved_api_key: bool,
    used_saved_athlete_id: bool,
) -> TestIntervalsConnectionResponse {
    TestIntervalsConnectionResponse {
        connected,
        message: message.to_string(),
        used_saved_api_key,
        used_saved_athlete_id,
        persisted_status_updated: false,
    }
}

fn map_connection_error_to_response(
    error: IntervalsConnectionError,
    used_saved_api_key: bool,
    used_saved_athlete_id: bool,
) -> Response {
    match error {
        IntervalsConnectionError::Unauthenticated => {
            log_connection_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            (
                StatusCode::BAD_REQUEST,
                Json(test_connection_response(
                    false,
                    "Invalid API key or athlete ID. Please check your credentials.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                )),
            )
                .into_response()
        }
        IntervalsConnectionError::InvalidConfiguration => {
            log_connection_error(Level::WARN, StatusCode::BAD_REQUEST, &error);
            (
                StatusCode::BAD_REQUEST,
                Json(test_connection_response(
                    false,
                    "Invalid configuration. Please check athlete ID.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                )),
            )
                .into_response()
        }
        IntervalsConnectionError::Unavailable => {
            log_connection_error(Level::ERROR, StatusCode::SERVICE_UNAVAILABLE, &error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(test_connection_response(
                    false,
                    "Intervals.icu is currently unavailable. Please try again later.",
                    used_saved_api_key,
                    used_saved_athlete_id,
                )),
            )
                .into_response()
        }
    }
}

fn log_settings_error(level: Level, status: StatusCode, error: &SettingsError) {
    let error_kind = match error {
        SettingsError::Repository(_) => "repository_error",
        SettingsError::Unauthenticated => "unauthenticated",
        SettingsError::Validation(_) => "validation_error",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "settings request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "settings request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn log_connection_error(level: Level, status: StatusCode, error: &IntervalsConnectionError) {
    let error_kind = match error {
        IntervalsConnectionError::InvalidConfiguration => "invalid_configuration",
        IntervalsConnectionError::Unavailable => "unavailable",
        IntervalsConnectionError::Unauthenticated => "unauthenticated",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "settings intervals connection test failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "settings intervals connection test failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

fn log_identity_error(level: Level, status: StatusCode, error: &IdentityError) {
    let error_kind = match error {
        IdentityError::Unauthenticated => "unauthenticated",
        IdentityError::Forbidden => "forbidden",
        IdentityError::Repository(_) => "repository_error",
        IdentityError::External(_) => "external_error",
        IdentityError::EmailNotVerified => "email_not_verified",
        IdentityError::InvalidLoginState => "invalid_login_state",
    };

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "admin identity request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error_kind,
            "admin identity request failed"
        ),
        _ => unreachable!("unexpected log level"),
    }
}

pub async fn test_intervals_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TestIntervalsConnectionRequest>,
) -> Response {
    let user_id = match super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    let settings_service = match state.settings_service.as_ref() {
        Some(s) => s,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let connection_tester = match state.intervals_connection_tester.as_ref() {
        Some(t) => t,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let current = match settings_service.get_settings(&user_id).await {
        Ok(s) => s,
        Err(err) => return map_settings_error(&err),
    };

    let transient_api_key = normalize_optional_input(body.api_key);
    let transient_athlete_id = normalize_optional_input(body.athlete_id);

    let transient_api_key_not_provided = transient_api_key.is_none();
    let transient_athlete_id_not_provided = transient_athlete_id.is_none();

    let merged = merge_credentials(
        transient_api_key,
        transient_athlete_id,
        current.intervals.api_key.clone(),
        current.intervals.athlete_id.clone(),
    );

    let (api_key, athlete_id, used_saved_api_key, used_saved_athlete_id) = match merged {
        Some(m) => (
            m.api_key,
            m.athlete_id,
            m.used_saved_api_key,
            m.used_saved_athlete_id,
        ),
        None => {
            return Json(test_connection_response(
                false,
                "Both API key and athlete ID are required.",
                transient_api_key_not_provided && current.intervals.api_key.is_some(),
                transient_athlete_id_not_provided && current.intervals.athlete_id.is_some(),
            ))
            .into_response();
        }
    };

    match connection_tester
        .test_connection(&api_key, &athlete_id)
        .await
    {
        Ok(_) => Json(test_connection_response(
            true,
            "Connection successful.",
            used_saved_api_key,
            used_saved_athlete_id,
        ))
        .into_response(),
        Err(e) => map_connection_error_to_response(e, used_saved_api_key, used_saved_athlete_id),
    }
}
