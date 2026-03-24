use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::Level;

use crate::{
    config::AppState,
    domain::intervals::{
        CreateEvent, DateRange, Event, EventCategory, IntervalsError, UpdateEvent,
    },
};

use super::logging::{format_error_chain, status_class};

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
    super::user_auth::resolve_user_id(state, headers).await
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
        IntervalsError::Unauthenticated => {
            log_intervals_error(Level::WARN, StatusCode::UNAUTHORIZED, &error);
            StatusCode::UNAUTHORIZED.into_response()
        }
        IntervalsError::CredentialsNotConfigured => (
            {
                log_intervals_error(Level::WARN, StatusCode::UNPROCESSABLE_ENTITY, &error);
                StatusCode::UNPROCESSABLE_ENTITY
            },
            "Intervals.icu credentials not configured",
        )
            .into_response(),
        IntervalsError::NotFound => {
            log_intervals_error(Level::WARN, StatusCode::NOT_FOUND, &error);
            StatusCode::NOT_FOUND.into_response()
        }
        IntervalsError::ApiError(_) | IntervalsError::ConnectionError(_) => {
            log_intervals_error(Level::ERROR, StatusCode::BAD_GATEWAY, &error);
            StatusCode::BAD_GATEWAY.into_response()
        }
        IntervalsError::Internal(_) => {
            log_intervals_error(Level::ERROR, StatusCode::INTERNAL_SERVER_ERROR, &error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn log_intervals_error(level: Level, status: StatusCode, error: &IntervalsError) {
    let error_chain = format_error_chain(error);

    match level {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class = status_class(status),
            error = %error,
            error_chain,
            "intervals request failed"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class = status_class(status),
            error = %error,
            error_chain,
            "intervals request failed"
        ),
        _ => unreachable!("unexpected log level"),
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
    EventCategory::from_str(category).ok()
}

fn is_valid_date(date: &str) -> bool {
    let mut segments = date.split('-');
    let (Some(year), Some(month), Some(day), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };

    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.chars().all(|ch| ch.is_ascii_digit())
        || !month.chars().all(|ch| ch.is_ascii_digit())
        || !day.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    let Ok(year) = year.parse::<i32>() else {
        return false;
    };
    let Ok(month) = month.parse::<u32>() else {
        return false;
    };
    let Ok(day) = day.parse::<u32>() else {
        return false;
    };

    let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year => 29,
        2 => 28,
        _ => return false,
    };

    (1..=max_day).contains(&day)
}

fn category_to_string(category: &EventCategory) -> String {
    category.as_str().to_string()
}
