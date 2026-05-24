use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::AppState;
use crate::domain::identity::IdentityError;
use crate::domain::task_scheduler::{
    RetryStrategy, ScheduledTask, TaskListFilter, TaskSchedulerError, TaskSortDirection,
    TaskSortField, TaskStatus, DEFAULT_TASK_LIST_LIMIT, MAX_TASK_LIST_LIMIT,
};

use super::cookies::read_cookie;
use super::same_origin::request_has_same_origin;

#[derive(Deserialize)]
pub struct TaskPath {
    task_id: String,
}

#[derive(Deserialize)]
pub struct TaskListQuery {
    limit: Option<usize>,
    offset: Option<usize>,
    #[serde(rename = "sortField")]
    sort_field: Option<String>,
    #[serde(rename = "sortDirection")]
    sort_direction: Option<String>,
}

#[derive(Serialize)]
pub struct TaskListResponse {
    items: Vec<TaskResponse>,
    #[serde(rename = "nextOffset")]
    next_offset: Option<usize>,
    #[serde(rename = "previousOffset")]
    previous_offset: Option<usize>,
    limit: usize,
}

#[derive(Serialize)]
pub struct TaskResponse {
    id: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "taskType")]
    task_type: String,
    status: String,
    payload: Value,
    checkpoint: Option<Value>,
    #[serde(rename = "retryStrategy")]
    retry_strategy: RetryStrategyResponse,
    #[serde(rename = "dedupeKey")]
    dedupe_key: String,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
    #[serde(rename = "attemptCount")]
    attempt_count: u32,
    #[serde(rename = "nextAttemptAtEpochSeconds")]
    next_attempt_at_epoch_seconds: i64,
    #[serde(rename = "claimedBy")]
    claimed_by: Option<String>,
    #[serde(rename = "leaseExpiresAtEpochSeconds")]
    lease_expires_at_epoch_seconds: Option<i64>,
    #[serde(rename = "lastHeartbeatAtEpochSeconds")]
    last_heartbeat_at_epoch_seconds: Option<i64>,
    #[serde(rename = "executionTimeoutSeconds")]
    execution_timeout_seconds: i64,
    #[serde(rename = "timedOutAtEpochSeconds")]
    timed_out_at_epoch_seconds: Option<i64>,
    #[serde(rename = "leaderOnly")]
    leader_only: bool,
    #[serde(rename = "createdAtEpochSeconds")]
    created_at_epoch_seconds: i64,
    #[serde(rename = "updatedAtEpochSeconds")]
    updated_at_epoch_seconds: i64,
    #[serde(rename = "startedAtEpochSeconds")]
    started_at_epoch_seconds: Option<i64>,
    #[serde(rename = "finishedAtEpochSeconds")]
    finished_at_epoch_seconds: Option<i64>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RetryStrategyResponse {
    Never,
    Fixed {
        #[serde(rename = "maxAttempts")]
        max_attempts: u32,
        #[serde(rename = "delaySeconds")]
        delay_seconds: i64,
    },
    Exponential {
        #[serde(rename = "maxAttempts")]
        max_attempts: u32,
        #[serde(rename = "initialDelaySeconds")]
        initial_delay_seconds: i64,
        #[serde(rename = "maxDelaySeconds")]
        max_delay_seconds: i64,
    },
}

pub async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TaskListQuery>,
) -> impl IntoResponse {
    match require_admin(&state, &headers).await {
        Ok(()) => {}
        Err(error) => return map_identity_error(&error).into_response(),
    }
    let Some(service) = state.admin_task_scheduler_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Ok(filter) = build_task_list_filter(query) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let limit = filter.clamped_limit();
    let offset = filter.offset;

    match service.list_tasks(filter).await {
        Ok(page) => Json(TaskListResponse {
            previous_offset: previous_offset(offset, limit),
            next_offset: page.has_next_page.then(|| offset.saturating_add(limit)),
            limit,
            items: page.tasks.into_iter().map(map_task_response).collect(),
        })
        .into_response(),
        Err(error) => map_task_scheduler_error(&error).into_response(),
    }
}

pub async fn get_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<TaskPath>,
) -> impl IntoResponse {
    match require_admin(&state, &headers).await {
        Ok(()) => {}
        Err(error) => return map_identity_error(&error).into_response(),
    }
    let Some(service) = state.admin_task_scheduler_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match service.get_task(&path.task_id).await {
        Ok(Some(task)) => Json(map_task_response(task)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => map_task_scheduler_error(&error).into_response(),
    }
}

pub async fn retry_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<TaskPath>,
) -> impl IntoResponse {
    match require_admin(&state, &headers).await {
        Ok(()) => {}
        Err(error) => return map_identity_error(&error).into_response(),
    }
    let Some(service) = state.admin_task_scheduler_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !request_has_same_origin(&headers, state.trust_proxy_headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match service.retry_task(&path.task_id).await {
        Ok(Some(task)) => Json(map_task_response(task)).into_response(),
        Ok(None) => map_retry_missing_or_conflict(service.as_ref(), &path.task_id).await,
        Err(error) => map_task_scheduler_error(&error).into_response(),
    }
}

async fn map_retry_missing_or_conflict(
    service: &dyn crate::domain::task_scheduler::AdminTaskSchedulerUseCases,
    task_id: &str,
) -> axum::response::Response {
    match service.get_task(task_id).await {
        Ok(Some(_)) => StatusCode::CONFLICT.into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => map_task_scheduler_error(&error).into_response(),
    }
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), IdentityError> {
    let Some(identity_service) = state.identity_service.clone() else {
        return Err(IdentityError::Repository(
            "identity service is not configured".to_string(),
        ));
    };
    let Some(session_id) = read_cookie(headers, &state.session_cookie_name) else {
        return Err(IdentityError::Unauthenticated);
    };
    identity_service
        .require_admin(&session_id)
        .await
        .map(|_| ())
}

fn build_task_list_filter(query: TaskListQuery) -> Result<TaskListFilter, ()> {
    Ok(TaskListFilter {
        limit: Some(
            query
                .limit
                .unwrap_or(DEFAULT_TASK_LIST_LIMIT)
                .min(MAX_TASK_LIST_LIMIT),
        ),
        offset: query.offset.unwrap_or(0),
        sort_field: query
            .sort_field
            .as_deref()
            .map(parse_sort_field)
            .transpose()?
            .unwrap_or_default(),
        sort_direction: query
            .sort_direction
            .as_deref()
            .map(parse_sort_direction)
            .transpose()?
            .unwrap_or_default(),
        ..TaskListFilter::default()
    })
}

fn previous_offset(offset: usize, limit: usize) -> Option<usize> {
    (offset > 0).then_some(offset.saturating_sub(limit))
}

fn parse_sort_field(value: &str) -> Result<TaskSortField, ()> {
    match value {
        "id" => Ok(TaskSortField::Id),
        "userId" => Ok(TaskSortField::UserId),
        "taskType" => Ok(TaskSortField::TaskType),
        "status" => Ok(TaskSortField::Status),
        "dedupeKey" => Ok(TaskSortField::DedupeKey),
        "errorMessage" => Ok(TaskSortField::ErrorMessage),
        "attemptCount" => Ok(TaskSortField::AttemptCount),
        "nextAttemptAt" => Ok(TaskSortField::NextAttemptAt),
        "claimedBy" => Ok(TaskSortField::ClaimedBy),
        "leaseExpiresAt" => Ok(TaskSortField::LeaseExpiresAt),
        "lastHeartbeatAt" => Ok(TaskSortField::LastHeartbeatAt),
        "executionTimeout" => Ok(TaskSortField::ExecutionTimeout),
        "timedOutAt" => Ok(TaskSortField::TimedOutAt),
        "leaderOnly" => Ok(TaskSortField::LeaderOnly),
        "createdAt" => Ok(TaskSortField::CreatedAt),
        "updatedAt" => Ok(TaskSortField::UpdatedAt),
        "startedAt" => Ok(TaskSortField::StartedAt),
        "finishedAt" => Ok(TaskSortField::FinishedAt),
        _ => Err(()),
    }
}

fn parse_sort_direction(value: &str) -> Result<TaskSortDirection, ()> {
    match value {
        "asc" => Ok(TaskSortDirection::Asc),
        "desc" => Ok(TaskSortDirection::Desc),
        _ => Err(()),
    }
}

fn map_task_response(task: ScheduledTask) -> TaskResponse {
    TaskResponse {
        id: task.id,
        user_id: task.user_id,
        task_type: task.task_type,
        status: status_as_str(&task.status).to_string(),
        payload: task.payload,
        checkpoint: task.checkpoint,
        retry_strategy: map_retry_strategy_response(task.retry_strategy),
        dedupe_key: task.dedupe_key,
        error_message: task.error_message,
        attempt_count: task.attempt_count,
        next_attempt_at_epoch_seconds: task.next_attempt_at_epoch_seconds,
        claimed_by: task.claimed_by,
        lease_expires_at_epoch_seconds: task.lease_expires_at_epoch_seconds,
        last_heartbeat_at_epoch_seconds: task.last_heartbeat_at_epoch_seconds,
        execution_timeout_seconds: task.execution_timeout_seconds,
        timed_out_at_epoch_seconds: task.timed_out_at_epoch_seconds,
        leader_only: task.leader_only,
        created_at_epoch_seconds: task.created_at_epoch_seconds,
        updated_at_epoch_seconds: task.updated_at_epoch_seconds,
        started_at_epoch_seconds: task.started_at_epoch_seconds,
        finished_at_epoch_seconds: task.finished_at_epoch_seconds,
    }
}

fn map_retry_strategy_response(strategy: RetryStrategy) -> RetryStrategyResponse {
    match strategy {
        RetryStrategy::Never => RetryStrategyResponse::Never,
        RetryStrategy::Fixed {
            max_attempts,
            delay_seconds,
        } => RetryStrategyResponse::Fixed {
            max_attempts,
            delay_seconds,
        },
        RetryStrategy::Exponential {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } => RetryStrategyResponse::Exponential {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        },
    }
}

fn status_as_str(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::RetryScheduled => "retry_scheduled",
        TaskStatus::Failed => "failed",
        TaskStatus::Completed => "completed",
        TaskStatus::TimedOut => "timed_out",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn map_identity_error(error: &IdentityError) -> StatusCode {
    match error {
        IdentityError::Unauthenticated => StatusCode::UNAUTHORIZED,
        IdentityError::Forbidden | IdentityError::EmailNotVerified => StatusCode::FORBIDDEN,
        IdentityError::Repository(_) | IdentityError::External(_) => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        IdentityError::InvalidLoginState => StatusCode::UNAUTHORIZED,
        IdentityError::InvalidEmail => StatusCode::BAD_REQUEST,
        IdentityError::PendingApproval => StatusCode::FORBIDDEN,
    }
}

fn map_task_scheduler_error(error: &TaskSchedulerError) -> StatusCode {
    match error {
        TaskSchedulerError::Validation(_) => StatusCode::BAD_REQUEST,
        TaskSchedulerError::Conflict(_) => StatusCode::CONFLICT,
        TaskSchedulerError::Repository(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
