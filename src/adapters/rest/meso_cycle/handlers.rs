use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::{
    config::AppState,
    domain::{
        ai_workflow::WorkflowStatus,
        meso_cycle::{MesoCycleError, MesoCycleOverlapStatus, MesoCycleUseCases},
    },
};

use super::{
    dto::{MesoCycleCalendarDayDto, MesoCycleOperationDto, MesoCycleStatusDto, MesoCycleWindowDto},
    error::map_meso_cycle_error,
};

#[derive(Deserialize)]
pub struct MesoCycleCalendarQuery {
    pub from: String,
    pub to: String,
}

pub async fn get_meso_cycle_status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let Some(service) = meso_cycle_service(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service.get_status(&user_id).await {
        Ok(status) => Json(map_status_to_dto(status)).into_response(),
        Err(error) => map_meso_cycle_error(&error),
    }
}

pub async fn get_meso_cycle_calendar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MesoCycleCalendarQuery>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let Some(service) = meso_cycle_service(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service
        .list_calendar_days(&user_id, &query.from, &query.to)
        .await
    {
        Ok(days) => Json(
            days.into_iter()
                .map(|day| MesoCycleCalendarDayDto {
                    date: day.date,
                    rest_day: day.rest_day,
                    rest_day_reason: day.rest_day_reason,
                    name: day.name,
                    raw_workout_doc: day.raw_workout_doc,
                    overlap_status: match day.overlap_status {
                        MesoCycleOverlapStatus::Active => "active".to_string(),
                        MesoCycleOverlapStatus::Outdated => "outdated".to_string(),
                    },
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => map_meso_cycle_error(&error),
    }
}

pub async fn post_generate_meso_cycle(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let Some(service) = meso_cycle_service(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service.generate_plan(&user_id).await {
        Ok(operation) => Json(map_operation_to_dto(operation)).into_response(),
        Err(error) => map_meso_cycle_error(&error),
    }
}

pub async fn get_meso_cycle_operation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(operation_key): Path<String>,
) -> Response {
    let user_id = match resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let Some(service) = meso_cycle_service(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service.get_operation(&user_id, &operation_key).await {
        Ok(operation) => Json(map_operation_to_dto(operation)).into_response(),
        Err(MesoCycleError::Validation(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => map_meso_cycle_error(&error),
    }
}

fn map_status_to_dto(status: crate::domain::meso_cycle::MesoCycleStatus) -> MesoCycleStatusDto {
    MesoCycleStatusDto {
        window: status.window.map(|window| MesoCycleWindowDto {
            meso_start: window.meso_start,
            meso_end: window.meso_end,
            ai_coach_last_date: window.ai_coach_last_date,
        }),
        has_pending_generation: status.has_pending_generation,
        latest_operation: status.latest_operation.map(map_operation_to_dto),
    }
}

fn map_operation_to_dto(
    operation: crate::domain::meso_cycle::MesoCycleGenerationOperation,
) -> MesoCycleOperationDto {
    MesoCycleOperationDto {
        operation_key: operation.operation_key,
        status: match operation.status {
            WorkflowStatus::Pending => "pending".to_string(),
            WorkflowStatus::Completed => "completed".to_string(),
            WorkflowStatus::Failed => "failed".to_string(),
        },
        meso_start: operation.meso_start,
        meso_end: operation.meso_end,
        failure_message: operation.failure.map(|failure| failure.message),
        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
    }
}

async fn resolve_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

fn meso_cycle_service(state: &AppState) -> Option<&Arc<dyn MesoCycleUseCases>> {
    state.meso_cycle_service.as_ref()
}
