use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::config::AppState;

use super::mapping::map_completed_workout_to_dto;

#[derive(Deserialize)]
pub(crate) struct ListCompletedWorkoutsQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub(crate) struct CompletedWorkoutPath {
    pub activity_id: String,
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

pub(crate) async fn list_completed_workouts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListCompletedWorkoutsQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    if !super::super::intervals::is_valid_date(&query.oldest)
        || !super::super::intervals::is_valid_date(&query.newest)
        || query.oldest > query.newest
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let service = match state.completed_workout_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service
        .list_completed_workouts(&user_id, &query.oldest, &query.newest)
        .await
    {
        Ok(workouts) => Json(
            workouts
                .into_iter()
                .map(map_completed_workout_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn get_completed_workout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CompletedWorkoutPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };

    let service = match state.completed_workout_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service
        .get_completed_workout(&user_id, &path.activity_id)
        .await
    {
        Ok(Some(workout)) => Json(map_completed_workout_to_dto(workout)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
