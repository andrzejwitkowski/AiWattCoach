use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::{
    config::AppState,
    domain::workout_summary::{
        CompletedWorkoutAliasScope, WorkoutSummaryGetOptions, WorkoutSummaryListOptions,
        WorkoutSummaryUseCases,
    },
};

use super::{
    dto::{
        GetWorkoutSummaryQuery, ListWorkoutSummariesQuery, SendMessageRequest,
        SetSavedStateRequest, UpdateRpeRequest, WorkoutSummaryPath, WorkoutSummaryStateResponse,
    },
    error::map_workout_summary_error,
    mapping::{
        map_save_summary_result_to_dto, map_send_message_result_to_dto,
        map_summary_metadata_to_dto, map_summary_to_dto, unchanged_save_summary_result,
    },
};

const MAX_LIST_SUMMARIES_WORKOUT_IDS: usize = 31;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(error: impl Into<String>) -> Json<ErrorResponse> {
    Json(ErrorResponse {
        error: error.into(),
    })
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

fn parse_alias_scope(
    oldest: Option<String>,
    newest: Option<String>,
) -> Option<CompletedWorkoutAliasScope> {
    let oldest = oldest?;
    let newest = newest?;
    if !super::super::intervals::is_valid_date(&oldest)
        || !super::super::intervals::is_valid_date(&newest)
        || oldest > newest
    {
        return None;
    }

    Some(CompletedWorkoutAliasScope { oldest, newest })
}

pub async fn get_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
    Query(query): Query<GetWorkoutSummaryQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match workout_summary_service(&state) {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let options = WorkoutSummaryGetOptions {
        alias_scope: parse_alias_scope(query.oldest, query.newest),
    };

    match service
        .get_summary_with_options(&user_id, &path.workout_id, options)
        .await
    {
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
            error_response("workoutIds must contain at least one workout id"),
        )
            .into_response();
    }

    if workout_ids.len() > MAX_LIST_SUMMARIES_WORKOUT_IDS {
        return (
            StatusCode::BAD_REQUEST,
            error_response(format!(
                "workoutIds must contain at most {} workout ids",
                MAX_LIST_SUMMARIES_WORKOUT_IDS
            )),
        )
            .into_response();
    }

    let metadata_only = query.view.as_deref() == Some("metadata");
    let options = WorkoutSummaryListOptions {
        alias_scope: parse_alias_scope(query.oldest, query.newest),
    };

    match service
        .list_summaries_with_options(&user_id, workout_ids, options)
        .await
    {
        Ok(summaries) => {
            let payload = if metadata_only {
                summaries
                    .into_iter()
                    .map(map_summary_metadata_to_dto)
                    .collect::<Vec<_>>()
            } else {
                summaries
                    .into_iter()
                    .map(map_summary_to_dto)
                    .collect::<Vec<_>>()
            };
            Json(payload).into_response()
        }
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
        let registered_save_notifier =
            state
                .workout_summary_save_notifier
                .as_ref()
                .inspect(|notifier| {
                    let _rx: tokio::sync::watch::Receiver<Option<super::dto::SaveWorkflowDto>> =
                        notifier.register(&user_id, &path.workout_id);
                });
        let result = service.mark_saved(&user_id, &path.workout_id).await;
        if result.is_err() {
            if let Some(notifier) = registered_save_notifier {
                notifier.unregister(&user_id, &path.workout_id);
            }
        }
        result
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

pub async fn get_power_chart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<WorkoutSummaryPath>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match state.completed_workout_service.as_ref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let workout = match service
        .get_completed_workout(&user_id, &path.workout_id)
        .await
    {
        Ok(Some(workout)) => workout,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match crate::domain::workout_summary::power_chart::extract_power_chart_data(&workout) {
        Some(data) => {
            let png = match tokio::task::spawn_blocking(move || {
                crate::domain::workout_summary::power_chart::render_power_chart_png(&data)
            })
            .await
            {
                Ok(png) => png,
                Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
            (StatusCode::OK, [("content-type", "image/png")], png).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
