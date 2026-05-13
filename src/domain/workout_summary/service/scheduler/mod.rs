use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{
        deserialize_llm_error, serialize_llm_error, LlmError, SerializedLlmError,
        LLM_REQUEST_TIMEOUT_SECONDS,
    },
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
        #[serde(flatten)]
        error: SerializedLlmError,
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
            error: serialize_llm_error(error),
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
        SerializedWorkoutSummaryError::Llm { error } => {
            WorkoutSummaryError::Llm(deserialize_llm_error(error))
        }
    }
}

pub use runner::workout_summary_coach_reply_task_handler;
pub use wrapper::SchedulerBackedWorkoutSummaryService;
