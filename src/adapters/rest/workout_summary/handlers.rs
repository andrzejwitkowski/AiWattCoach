use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::{config::AppState, domain::workout_summary::WorkoutSummaryUseCases};

use super::{
    dto::{
        ListWorkoutSummariesQuery, SendMessageRequest, SetSavedStateRequest, UpdateRpeRequest,
        WorkoutSummaryPath, WorkoutSummaryStateResponse,
    },
    error::map_workout_summary_error,
    mapping::{
        map_save_summary_result_to_dto, map_send_message_result_to_dto, map_summary_to_dto,
        unchanged_save_summary_result,
    },
};

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub(super) async fn resolve_user_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

fn workout_summary_service(
    state: &AppState,
) -> Option<&std::sync::Arc<dyn WorkoutSummaryUseCases>> {
    state.workout_summary_service.as_ref()
}

pub async fn get_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service.get_summary(&user_id, &path.workout_id).await {
        Ok(summary) => Json(map_summary_to_dto(summary)).into_response(),
        Err(error) => map_workout_summary_error(&error),
    }
}

pub async fn create_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service.create_summary(&user_id, &path.workout_id).await {
        Ok(summary) => (StatusCode::CREATED, Json(map_summary_to_dto(summary))).into_response(),
        Err(error) => map_workout_summary_error(&error),
    }
}

pub async fn list_summaries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListWorkoutSummariesQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let workout_ids = query
        .workout_ids
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if workout_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "workoutIds must contain at least one workout id",
            }),
        )
            .into_response();
    }

    match service.list_summaries(&user_id, workout_ids).await {
        Ok(summaries) => Json(
            summaries
                .into_iter()
                .map(map_summary_to_dto)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_workout_summary_error(&error),
    }
}

pub async fn update_rpe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
    Json(body): Json<UpdateRpeRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service
        .update_rpe(&user_id, &path.workout_id, body.rpe)
        .await
    {
        Ok(summary) => Json(map_summary_to_dto(summary)).into_response(),
        Err(error) => map_workout_summary_error(&error),
    }
}

pub async fn set_saved_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
    Json(body): Json<SetSavedStateRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let result = if body.saved {
        service.mark_saved(&user_id, &path.workout_id).await
    } else {
        service
            .reopen_summary(&user_id, &path.workout_id)
            .await
            .map(unchanged_save_summary_result)
    };

    match result {
        Ok(result) => {
            let (summary, workflow) = map_save_summary_result_to_dto(result);
            Json(WorkoutSummaryStateResponse { summary, workflow }).into_response()
        }
        Err(error) => map_workout_summary_error(&error),
    }
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
    Json(body): Json<SendMessageRequest>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    match service
        .send_message(&user_id, &path.workout_id, body.content)
        .await
    {
        Ok(result) => Json(map_send_message_result_to_dto(result)).into_response(),
        Err(error) => map_workout_summary_error(&error),
    }
}
