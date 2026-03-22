use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    config::AppState,
    domain::intervals::{
        CreateEvent, DateRange, Event, EventCategory, IntervalsError, UpdateEvent,
    },
};

use super::cookies::read_cookie;

#[derive(Deserialize)]
pub struct ListEventsQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub struct EventPath {
    pub event_id: i64,
}

#[derive(Serialize)]
pub struct EventDto {
    pub id: i64,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "eventDefinition")]
    pub event_definition: EventDefinitionDto,
    #[serde(rename = "actualWorkout")]
    pub actual_workout: Option<ActualWorkoutDto>,
}

#[derive(Serialize)]
pub struct EventDefinitionDto {
    #[serde(rename = "rawWorkoutDoc")]
    pub raw_workout_doc: Option<String>,
    pub intervals: Vec<IntervalDefinitionDto>,
}

#[derive(Serialize)]
pub struct IntervalDefinitionDto {
    pub definition: String,
}

#[derive(Serialize)]
pub struct ActualWorkoutDto {
    #[serde(rename = "powerValues")]
    pub power_values: Vec<i32>,
    #[serde(rename = "cadenceValues")]
    pub cadence_values: Vec<i32>,
    #[serde(rename = "heartRateValues")]
    pub heart_rate_values: Vec<i32>,
}

#[derive(Deserialize)]
pub struct CreateEventDto {
    pub category: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateEventDto {
    pub category: Option<String>,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    let identity_service = state
        .identity_service
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;

    let session_id = read_cookie(headers, &state.session_cookie_name)
        .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

    let user = identity_service
        .get_current_user(&session_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())?
        .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

    Ok(user.id)
}

pub async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListEventsQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !is_valid_date(&range.oldest) || !is_valid_date(&range.newest) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match intervals_service.list_events(&user_id, &range).await {
        Ok(events) => {
            Json(events.into_iter().map(map_event_to_dto).collect::<Vec<_>>()).into_response()
        }
        Err(error) => map_intervals_error(error),
    }
}

pub async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service.get_event(&user_id, path.event_id).await {
        Ok(event) => Json(map_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateEventDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let category = match parse_category(body.category.as_str()) {
        Some(category) => category,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    if !is_valid_date(&body.start_date_local) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let event = CreateEvent {
        category,
        start_date_local: body.start_date_local,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
    };

    match intervals_service.create_event(&user_id, event).await {
        Ok(event) => (StatusCode::CREATED, Json(map_event_to_dto(event))).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn update_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
    Json(body): Json<UpdateEventDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let category = match body.category.as_deref() {
        Some(category) => match parse_category(category) {
            Some(category) => Some(category),
            None => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => None,
    };

    if let Some(start_date_local) = body.start_date_local.as_deref() {
        if !is_valid_date(start_date_local) {
            return StatusCode::BAD_REQUEST.into_response();
        }
    }

    let event = UpdateEvent {
        category,
        start_date_local: body.start_date_local,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
    };

    match intervals_service
        .update_event(&user_id, path.event_id, event)
        .await
    {
        Ok(event) => Json(map_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn delete_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .delete_event(&user_id, path.event_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub async fn download_fit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<EventPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match intervals_service
        .download_fit(&user_id, path.event_id)
        .await
    {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"event-{}.fit\"", path.event_id),
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(error) => map_intervals_error(error),
    }
}

fn map_intervals_error(error: IntervalsError) -> Response {
    match error {
        IntervalsError::Unauthenticated => StatusCode::UNAUTHORIZED.into_response(),
        IntervalsError::CredentialsNotConfigured => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Intervals.icu credentials not configured",
        )
            .into_response(),
        IntervalsError::NotFound => StatusCode::NOT_FOUND.into_response(),
        IntervalsError::ApiError(_) | IntervalsError::ConnectionError(_) => {
            StatusCode::BAD_GATEWAY.into_response()
        }
        IntervalsError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn map_event_to_dto(event: Event) -> EventDto {
    EventDto {
        id: event.id,
        start_date_local: event.start_date_local,
        name: event.name,
        category: category_to_string(&event.category),
        description: event.description,
        indoor: event.indoor,
        color: event.color,
        event_definition: EventDefinitionDto {
            raw_workout_doc: event.workout_doc.clone(),
            intervals: parse_workout_doc(event.workout_doc.as_deref()),
        },
        actual_workout: None,
    }
}

fn parse_workout_doc(workout_doc: Option<&str>) -> Vec<IntervalDefinitionDto> {
    workout_doc
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| IntervalDefinitionDto {
            definition: line.to_string(),
        })
        .collect()
}

fn parse_category(category: &str) -> Option<EventCategory> {
    match category {
        "WORKOUT" => Some(EventCategory::Workout),
        "RACE" => Some(EventCategory::Race),
        "NOTE" => Some(EventCategory::Note),
        "TARGET" => Some(EventCategory::Target),
        "SEASON" => Some(EventCategory::Season),
        "OTHER" => Some(EventCategory::Other),
        _ => None,
    }
}

fn is_valid_date(date: &str) -> bool {
    if date.len() != 10 {
        return false;
    }

    let bytes = date.as_bytes();
    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn category_to_string(category: &EventCategory) -> String {
    match category {
        EventCategory::Workout => "WORKOUT".to_string(),
        EventCategory::Race => "RACE".to_string(),
        EventCategory::Note => "NOTE".to_string(),
        EventCategory::Target => "TARGET".to_string(),
        EventCategory::Season => "SEASON".to_string(),
        EventCategory::Other => "OTHER".to_string(),
    }
}
