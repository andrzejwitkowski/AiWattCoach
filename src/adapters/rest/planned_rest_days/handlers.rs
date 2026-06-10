use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    config::AppState,
    domain::{
        intervals::DateRange,
        planned_rest_days::{CreatePlannedRestDay, PlannedRestDayUseCases, UpdatePlannedRestDay},
    },
};

use super::{
    dto::{ListPlannedRestDaysQuery, PlannedRestDayPath, UpsertPlannedRestDayRequest},
    error::map_planned_rest_day_error,
    mapping::map_planned_rest_day_to_dto,
};

async fn auth_and_get_service<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
) -> Result<(String, &'a dyn PlannedRestDayUseCases), Response> {
    let user_id = super::super::user_auth::resolve_user_id(state, headers).await?;
    let service = state
        .planned_rest_day_service
        .as_deref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    Ok((user_id, service))
}

pub(in crate::adapters::rest) async fn list_planned_rest_days(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListPlannedRestDaysQuery>,
) -> Response {
    let (user_id, service) = match auth_and_get_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    let range = DateRange {
        oldest: query.oldest,
        newest: query.newest,
    };

    if !super::super::intervals::is_valid_date(&range.oldest)
        || !super::super::intervals::is_valid_date(&range.newest)
        || range.oldest > range.newest
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match service.list(&user_id, &range).await {
        Ok(entries) => Json(
            entries
                .into_iter()
                .map(map_planned_rest_day_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_planned_rest_day_error(error),
    }
}

pub(in crate::adapters::rest) async fn get_planned_rest_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<PlannedRestDayPath>,
) -> Response {
    let (user_id, service) = match auth_and_get_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    match service.get(&user_id, &path.planned_rest_day_id).await {
        Ok(entry) => Json(map_planned_rest_day_to_dto(entry)).into_response(),
        Err(error) => map_planned_rest_day_error(error),
    }
}

pub(in crate::adapters::rest) async fn create_planned_rest_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpsertPlannedRestDayRequest>,
) -> Response {
    let (user_id, service) = match auth_and_get_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    let request = match map_request(body) {
        Ok(request) => request,
        Err(status) => return status.into_response(),
    };

    match service.create(&user_id, request.into()).await {
        Ok(entry) => (
            StatusCode::CREATED,
            Json(map_planned_rest_day_to_dto(entry)),
        )
            .into_response(),
        Err(error) => map_planned_rest_day_error(error),
    }
}

pub(in crate::adapters::rest) async fn update_planned_rest_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<PlannedRestDayPath>,
    Json(body): Json<UpsertPlannedRestDayRequest>,
) -> Response {
    let (user_id, service) = match auth_and_get_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    let request = match map_request(body) {
        Ok(request) => request,
        Err(status) => return status.into_response(),
    };

    match service
        .update(&user_id, &path.planned_rest_day_id, request)
        .await
    {
        Ok(entry) => Json(map_planned_rest_day_to_dto(entry)).into_response(),
        Err(error) => map_planned_rest_day_error(error),
    }
}

pub(in crate::adapters::rest) async fn delete_planned_rest_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<PlannedRestDayPath>,
) -> Response {
    let (user_id, service) = match auth_and_get_service(&state, &headers).await {
        Ok(pair) => pair,
        Err(response) => return response,
    };

    match service.delete(&user_id, &path.planned_rest_day_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_planned_rest_day_error(error),
    }
}

fn map_request(body: UpsertPlannedRestDayRequest) -> Result<UpdatePlannedRestDay, StatusCode> {
    if !super::super::intervals::is_valid_date(&body.start_date)
        || !super::super::intervals::is_valid_date(&body.end_date)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(UpdatePlannedRestDay {
        start_date: body.start_date,
        end_date: body.end_date,
        title: body.title,
        note: body.note,
    })
}

impl From<UpdatePlannedRestDay> for CreatePlannedRestDay {
    fn from(value: UpdatePlannedRestDay) -> Self {
        Self {
            start_date: value.start_date,
            end_date: value.end_date,
            title: value.title,
            note: value.note,
        }
    }
}
