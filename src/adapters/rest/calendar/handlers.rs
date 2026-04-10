use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::adapters::rest::intervals::is_valid_date;
use crate::{
    config::AppState,
    domain::{calendar::SyncPlannedWorkout, intervals::DateRange},
};

use super::{
    dto::{ListCalendarEventsQuery, SyncPlannedWorkoutPath},
    error::{map_calendar_error, map_calendar_label_error},
    mapping::{map_calendar_event_to_dto, map_calendar_labels_to_dto},
};

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

pub(in crate::adapters::rest) async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListCalendarEventsQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let calendar_service = match state.calendar_service.as_ref() {
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
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let calendar_labels_service = match state.calendar_labels_service.as_ref() {
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
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let calendar_service = match state.calendar_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
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
