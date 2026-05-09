use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{LlmError, LLM_REQUEST_TIMEOUT_SECONDS},
    task_scheduler::{
        build_scheduled_task, scheduled_task_handler, BuildScheduledTaskError,
        NewScheduledTaskInput, RetryStrategy, ScheduledTask, ScheduledTaskExecutor,
        SharedTaskHandler, TaskRepository, TaskRunOutcome, TaskSchedulerError,
        TaskSchedulerService, TaskWorkerRepository,
    },
    workout_summary::{
        CoachReply, ConversationMessage, PersistedUserMessage, SendMessageResult, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryUseCases,
    },
};

use super::BoxFuture;

mod checkpoint;
mod runner;
mod wrapper;

#[cfg(test)]
mod tests;

pub(crate) const COACH_REPLY_TASK_TYPE: &str = "workout_summary.coach_reply";
pub(crate) const COACH_REPLY_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const COACH_REPLY_EXECUTION_TIMEOUT_SECONDS: i64 =
    (LLM_REQUEST_TIMEOUT_SECONDS as i64 * 2) + COACH_REPLY_EXECUTION_TIMEOUT_BUFFER_SECONDS;
pub(crate) const COACH_REPLY_LEASE_DURATION_SECONDS: i64 = 30;
pub(crate) const COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS: u64 = 10;
pub(crate) const COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WorkoutSummaryCoachReplyTaskPayload {
    pub(crate) user_id: String,
    pub(crate) workout_id: String,
    pub(crate) user_message_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SerializedWorkoutSummaryError {
    AlreadyExists,
    Locked,
    NotFound,
    ReplyAlreadyPending,
    Repository {
        message: String,
    },
    Validation {
        message: String,
    },
    Llm {
        error_kind: String,
        message: Option<String>,
    },
}

pub(crate) fn coach_reply_dedupe_key(
    user_id: &str,
    workout_id: &str,
    user_message_id: &str,
) -> String {
    format!("workout-summary:{user_id}:{workout_id}:{user_message_id}")
}

pub(crate) fn map_task_scheduler_error(error: TaskSchedulerError) -> WorkoutSummaryError {
    match error {
        TaskSchedulerError::Validation(message)
        | TaskSchedulerError::Conflict(message)
        | TaskSchedulerError::Repository(message) => WorkoutSummaryError::Repository(message),
    }
}

pub(crate) fn serialize_workout_summary_error(
    error: &WorkoutSummaryError,
) -> SerializedWorkoutSummaryError {
    match error {
        WorkoutSummaryError::AlreadyExists => SerializedWorkoutSummaryError::AlreadyExists,
        WorkoutSummaryError::Locked => SerializedWorkoutSummaryError::Locked,
        WorkoutSummaryError::NotFound => SerializedWorkoutSummaryError::NotFound,
        WorkoutSummaryError::ReplyAlreadyPending => {
            SerializedWorkoutSummaryError::ReplyAlreadyPending
        }
        WorkoutSummaryError::Repository(message) => SerializedWorkoutSummaryError::Repository {
            message: message.clone(),
        },
        WorkoutSummaryError::Validation(message) => SerializedWorkoutSummaryError::Validation {
            message: message.clone(),
        },
        WorkoutSummaryError::Llm(error) => SerializedWorkoutSummaryError::Llm {
            error_kind: match error {
                LlmError::CredentialsNotConfigured => "credentials_not_configured",
                LlmError::ProviderNotConfigured => "provider_not_configured",
                LlmError::ModelNotConfigured => "model_not_configured",
                LlmError::ContextTooLarge(_) => "context_too_large",
                LlmError::UnsupportedProvider(_) => "unsupported_provider",
                LlmError::Transport(_) => "transport",
                LlmError::ProviderRejected(_) => "provider_rejected",
                LlmError::RateLimited(_) => "rate_limited",
                LlmError::InvalidResponse(_) => "invalid_response",
                LlmError::Checkpoint(_) => "checkpoint",
                LlmError::Internal(_) => "internal",
            }
            .to_string(),
            message: match error {
                LlmError::CredentialsNotConfigured
                | LlmError::ProviderNotConfigured
                | LlmError::ModelNotConfigured => None,
                LlmError::ContextTooLarge(message)
                | LlmError::UnsupportedProvider(message)
                | LlmError::Transport(message)
                | LlmError::ProviderRejected(message)
                | LlmError::RateLimited(message)
                | LlmError::InvalidResponse(message)
                | LlmError::Checkpoint(message)
                | LlmError::Internal(message) => Some(message.clone()),
            },
        },
    }
}

pub(crate) fn deserialize_workout_summary_error(
    error: SerializedWorkoutSummaryError,
) -> WorkoutSummaryError {
    match error {
        SerializedWorkoutSummaryError::AlreadyExists => WorkoutSummaryError::AlreadyExists,
        SerializedWorkoutSummaryError::Locked => WorkoutSummaryError::Locked,
        SerializedWorkoutSummaryError::NotFound => WorkoutSummaryError::NotFound,
        SerializedWorkoutSummaryError::ReplyAlreadyPending => {
            WorkoutSummaryError::ReplyAlreadyPending
        }
        SerializedWorkoutSummaryError::Repository { message } => {
            WorkoutSummaryError::Repository(message)
        }
        SerializedWorkoutSummaryError::Validation { message } => {
            WorkoutSummaryError::Validation(message)
        }
        SerializedWorkoutSummaryError::Llm {
            error_kind,
            message,
        } => WorkoutSummaryError::Llm(match error_kind.as_str() {
            "credentials_not_configured" => LlmError::CredentialsNotConfigured,
            "provider_not_configured" => LlmError::ProviderNotConfigured,
            "model_not_configured" => LlmError::ModelNotConfigured,
            "context_too_large" => LlmError::ContextTooLarge(
                message
                    .unwrap_or_else(|| "packed training context exceeds model limits".to_string()),
            ),
            "unsupported_provider" => LlmError::UnsupportedProvider(
                message.unwrap_or_else(|| "unknown provider".to_string()),
            ),
            "transport" => {
                LlmError::Transport(message.unwrap_or_else(|| "transport error".to_string()))
            }
            "provider_rejected" => LlmError::ProviderRejected(
                message.unwrap_or_else(|| "provider rejected request".to_string()),
            ),
            "rate_limited" => LlmError::RateLimited(
                message.unwrap_or_else(|| "provider rate limited request".to_string()),
            ),
            "invalid_response" => LlmError::InvalidResponse(
                message.unwrap_or_else(|| "invalid provider response".to_string()),
            ),
            _ => LlmError::Internal(message.unwrap_or_else(|| "internal llm error".to_string())),
        }),
    }
}

pub use runner::workout_summary_coach_reply_task_handler;
pub use wrapper::SchedulerBackedWorkoutSummaryService;
