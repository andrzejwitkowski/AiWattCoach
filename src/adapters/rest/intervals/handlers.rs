use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    config::AppState,
    domain::intervals::{CreateEvent, DateRange, UpdateActivity, UpdateEvent, UploadActivity},
};

use super::{
    dto::{
        ActivityPath, CreateActivityDto, CreateEventDto, EventPath, ListEventsQuery,
        UpdateActivityDto, UpdateEventDto, UploadActivityResponseDto,
    },
    error::map_intervals_error,
    mapping::{map_activity_to_dto, map_enriched_event_to_dto, map_event_to_dto},
    validation::{decode_base64, is_valid_date, parse_category, try_map_event_file_upload},
};

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

pub(in crate::adapters::rest) async fn list_events(
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

pub(in crate::adapters::rest) async fn get_event(
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
        .get_enriched_event(&user_id, path.event_id)
        .await
    {
        Ok(event) => Json(map_enriched_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn create_event(
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
        event_type: body.event_type,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
        file_upload: match try_map_event_file_upload(body.file_upload) {
            Ok(file_upload) => file_upload,
            Err(status) => return status.into_response(),
        },
    };

    match intervals_service.create_event(&user_id, event).await {
        Ok(event) => (StatusCode::CREATED, Json(map_event_to_dto(event))).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn update_event(
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
        event_type: body.event_type,
        name: body.name,
        description: body.description,
        indoor: body.indoor,
        color: body.color,
        workout_doc: body.workout_doc,
        file_upload: match try_map_event_file_upload(body.file_upload) {
            Ok(file_upload) => file_upload,
            Err(status) => return status.into_response(),
        },
    };

    match intervals_service
        .update_event(&user_id, path.event_id, event)
        .await
    {
        Ok(event) => Json(map_event_to_dto(event)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn delete_event(
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

pub(in crate::adapters::rest) async fn download_fit(
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

pub(in crate::adapters::rest) async fn list_activities(
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

    match intervals_service.list_activities(&user_id, &range).await {
        Ok(activities) => Json(
            activities
                .into_iter()
                .map(map_activity_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn get_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
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
        .get_activity(&user_id, &path.activity_id)
        .await
    {
        Ok(activity) => Json(map_activity_to_dto(activity)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn create_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateActivityDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let file_bytes = match decode_base64(&body.file_contents_base64) {
        Ok(bytes) => bytes,
        Err(()) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let upload = UploadActivity {
        filename: body.filename,
        file_bytes,
        name: body.name,
        description: body.description,
        device_name: body.device_name,
        external_id: body.external_id,
        paired_event_id: body.paired_event_id,
    };

    match intervals_service.upload_activity(&user_id, upload).await {
        Ok(result) => (
            if result.created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(UploadActivityResponseDto {
                created: result.created,
                activity_ids: result.activity_ids,
                activities: result
                    .activities
                    .into_iter()
                    .map(map_activity_to_dto)
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn update_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
    Json(body): Json<UpdateActivityDto>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let intervals_service = match state.intervals_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let update = UpdateActivity {
        name: body.name,
        description: body.description,
        activity_type: body.activity_type,
        trainer: body.trainer,
        commute: body.commute,
        race: body.race,
    };

    match intervals_service
        .update_activity(&user_id, &path.activity_id, update)
        .await
    {
        Ok(activity) => Json(map_activity_to_dto(activity)).into_response(),
        Err(error) => map_intervals_error(error),
    }
}

pub(in crate::adapters::rest) async fn delete_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<ActivityPath>,
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
        .delete_activity(&user_id, &path.activity_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_intervals_error(error),
    }
}
