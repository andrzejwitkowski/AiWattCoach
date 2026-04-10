use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::adapters::rest::intervals::is_valid_date;
use crate::{
    config::AppState,
    domain::{
        calendar::{CalendarUseCases, SyncPlannedWorkout},
        calendar_labels::CalendarLabelsUseCases,
        intervals::DateRange,
    },
};

use super::{
    dto::{ListCalendarEventsQuery, SyncPlannedWorkoutPath},
    error::{map_calendar_error, map_calendar_label_error},
    mapping::{map_calendar_event_to_dto, map_calendar_labels_to_dto},
};

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

async fn auth_and_get_calendar_service<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
) -> Result<(String, &'a dyn CalendarUseCases), Response> {
    let user_id = resolve_user_id(state, headers).await?;
    let service = state
        .calendar_service
        .as_deref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    Ok((user_id, service))
}

async fn auth_and_get_calendar_labels_service<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
) -> Result<(String, &'a dyn CalendarLabelsUseCases), Response> {
    let user_id = resolve_user_id(state, headers).await?;
    let service = state
        .calendar_labels_service
        .as_deref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    Ok((user_id, service))
}

pub(in crate::adapters::rest) async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListCalendarEventsQuery>,
) -> Response {
    let (user_id, calendar_service) = match auth_and_get_calendar_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !is_valid_date(&range.oldest) || !is_valid_date(&range.newest) || range.oldest > range.newest
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match calendar_service.list_events(&user_id, &range).await {
        Ok(events) => Json(
            events
                .into_iter()
                .map(map_calendar_event_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_calendar_error(error),
    }
}

pub(in crate::adapters::rest) async fn list_labels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListCalendarEventsQuery>,
) -> Response {
    let (user_id, calendar_labels_service) =
        match auth_and_get_calendar_labels_service(&state, &headers).await {
            Ok(pair) => pair,
            Err(response) => return response,
        };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !is_valid_date(&range.oldest) || !is_valid_date(&range.newest) || range.oldest > range.newest
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match calendar_labels_service.list_labels(&user_id, &range).await {
        Ok(labels) => Json(map_calendar_labels_to_dto(labels)).into_response(),
        Err(error) => map_calendar_label_error(error),
    }
}

pub(in crate::adapters::rest) async fn sync_planned_workout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<SyncPlannedWorkoutPath>,
) -> Response {
    let (user_id, calendar_service) = match auth_and_get_calendar_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    if !is_valid_date(&path.date) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match calendar_service
        .sync_planned_workout(
            &user_id,
            SyncPlannedWorkout {
                operation_key: path.operation_key,
                date: path.date,
            },
        )
        .await
    {
        Ok(event) => (StatusCode::OK, Json(map_calendar_event_to_dto(event))).into_response(),
        Err(error) => map_calendar_error(error),
    }
}
