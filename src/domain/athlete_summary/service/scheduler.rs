use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{
        deserialize_llm_error, serialize_llm_error, SerializedLlmError, LLM_REQUEST_TIMEOUT_SECONDS,
    },
    task_scheduler::{
        build_scheduled_task, parse_failed_or_error_message, parse_optional_json_value,
        parse_required_json_value, scheduled_task_handler, serialize_json_value,
        BuildScheduledTaskError, NewScheduledTaskInput, ResultTaskHandler, RetryStrategy,
        ScheduledTask, ScheduledTaskExecutor, SharedTaskHandler, TaskRepository, TaskRunOutcome,
        TaskSchedulerError, TaskSchedulerService, TaskWorkerRepository,
    },
};

use super::super::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryState, BoxFuture, EnsuredAthleteSummary,
};
use super::core::{
    current_week_monday_epoch_seconds, AthleteSummaryUseCases, GENERATION_ALREADY_PENDING_MESSAGE,
    STALE_PENDING_TIMEOUT_SECONDS,
};

pub(crate) const ATHLETE_SUMMARY_GENERATE_TASK_TYPE: &str = "athlete_summary.generate";
pub(crate) const ATHLETE_SUMMARY_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const ATHLETE_SUMMARY_EXECUTION_TIMEOUT_SECONDS: i64 =
    LLM_REQUEST_TIMEOUT_SECONDS as i64 + ATHLETE_SUMMARY_EXECUTION_TIMEOUT_BUFFER_SECONDS;
pub(crate) const ATHLETE_SUMMARY_RETRY_MAX_ATTEMPTS: u32 = 3;
pub(crate) const ATHLETE_SUMMARY_RETRY_DELAY_SECONDS: i64 = 30;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AthleteSummaryTaskPayload {
    user_id: String,
    force: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SerializedAthleteSummaryError {
    NotConfigured,
    Unavailable {
        message: String,
    },
    Repository {
        message: String,
    },
    Llm {
        #[serde(flatten)]
        error: SerializedLlmError,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletedAthleteSummaryTaskCheckpoint {
    user_id: String,
}

#[cfg(test)]
mod tests;

fn map_task_scheduler_error(error: TaskSchedulerError) -> AthleteSummaryError {
    match error {
        TaskSchedulerError::Validation(message)
        | TaskSchedulerError::Conflict(message)
        | TaskSchedulerError::Repository(message) => AthleteSummaryError::Repository(message),
    }
}

fn serialize_athlete_summary_error(error: &AthleteSummaryError) -> SerializedAthleteSummaryError {
    match error {
        AthleteSummaryError::NotConfigured => SerializedAthleteSummaryError::NotConfigured,
        AthleteSummaryError::Unavailable(message) => SerializedAthleteSummaryError::Unavailable {
            message: message.clone(),
        },
        AthleteSummaryError::Repository(message) => SerializedAthleteSummaryError::Repository {
            message: message.clone(),
        },
        AthleteSummaryError::Llm(error) => SerializedAthleteSummaryError::Llm {
            error: serialize_llm_error(error),
        },
    }
}

fn deserialize_athlete_summary_error(error: SerializedAthleteSummaryError) -> AthleteSummaryError {
    match error {
        SerializedAthleteSummaryError::NotConfigured => AthleteSummaryError::NotConfigured,
        SerializedAthleteSummaryError::Unavailable { message } => {
            AthleteSummaryError::Unavailable(message)
        }
        SerializedAthleteSummaryError::Repository { message } => {
            AthleteSummaryError::Repository(message)
        }
        SerializedAthleteSummaryError::Llm { error } => {
            AthleteSummaryError::Llm(deserialize_llm_error(error))
        }
    }
}

fn parse_failed_checkpoint(
    task: &ScheduledTask,
) -> Result<Option<AthleteSummaryError>, AthleteSummaryError> {
    parse_optional_json_value::<SerializedAthleteSummaryError, AthleteSummaryError>(
        task.checkpoint.clone(),
        "invalid failed athlete summary checkpoint",
        AthleteSummaryError::Repository,
    )
    .map(|error| error.map(deserialize_athlete_summary_error))
}

fn build_non_force_dedupe_key(user_id: &str, refresh_window_start_epoch_seconds: i64) -> String {
    format!("athlete-summary:{user_id}:{refresh_window_start_epoch_seconds}")
}

fn build_force_dedupe_key(user_id: &str, task_id: &str) -> String {
    format!("athlete-summary:force:{user_id}:{task_id}")
}

fn build_completed_checkpoint(user_id: &str) -> Result<serde_json::Value, AthleteSummaryError> {
    serialize_json_value(
        &CompletedAthleteSummaryTaskCheckpoint {
            user_id: user_id.to_string(),
        },
        "failed to serialize completed athlete summary checkpoint",
        AthleteSummaryError::Repository,
    )
}

struct AthleteSummaryGenerateTaskExecutor<Base> {
    base: Arc<Base>,
}

impl<Base> AthleteSummaryGenerateTaskExecutor<Base>
where
    Base: AthleteSummaryUseCases + 'static,
{
    fn map_task_failure(error: AthleteSummaryError) -> TaskRunOutcome {
        let (retryable, retry_delay_seconds) = match &error {
            AthleteSummaryError::Llm(llm_error) => (llm_error.is_retryable(), None),
            AthleteSummaryError::Repository(_) => (true, None),
            AthleteSummaryError::Unavailable(message)
                if message == GENERATION_ALREADY_PENDING_MESSAGE =>
            {
                (true, Some(STALE_PENDING_TIMEOUT_SECONDS))
            }
            AthleteSummaryError::NotConfigured | AthleteSummaryError::Unavailable(_) => {
                (false, None)
            }
        };

        TaskRunOutcome::Failed {
            checkpoint: serde_json::to_value(serialize_athlete_summary_error(&error)).ok(),
            error_message: error.to_string(),
            retryable,
            retry_delay_seconds,
        }
    }
}

impl<Base> ScheduledTaskExecutor for AthleteSummaryGenerateTaskExecutor<Base>
where
    Base: AthleteSummaryUseCases + 'static,
{
    type Payload = AthleteSummaryTaskPayload;
    type Output = AthleteSummary;
    type Error = AthleteSummaryError;

    fn task_type(&self) -> &'static str {
        ATHLETE_SUMMARY_GENERATE_TASK_TYPE
    }

    fn parse_error(&self, error: serde_json::Error) -> Self::Error {
        AthleteSummaryError::Repository(format!(
            "invalid athlete summary generate task payload: {error}"
        ))
    }

    fn run(
        &self,
        _task: ScheduledTask,
        payload: Self::Payload,
    ) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move { base.generate_summary(&payload.user_id, payload.force).await })
    }

    fn completed_checkpoint(
        &self,
        _task_id: &str,
        output: &Self::Output,
    ) -> Result<Option<serde_json::Value>, Self::Error> {
        build_completed_checkpoint(&output.user_id).map(Some)
    }

    fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome {
        Self::map_task_failure(error)
    }
}

pub fn athlete_summary_generate_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: AthleteSummaryUseCases + 'static,
{
    Arc::new(scheduled_task_handler(AthleteSummaryGenerateTaskExecutor {
        base,
    }))
}

#[derive(Clone)]
struct AthleteSummaryTaskResultHandler<Base> {
    base: Arc<Base>,
    user_id: String,
}

impl<Base> ResultTaskHandler for AthleteSummaryTaskResultHandler<Base>
where
    Base: AthleteSummaryUseCases + 'static,
{
    type Completed = CompletedAthleteSummaryTaskCheckpoint;
    type Output = AthleteSummary;
    type Error = AthleteSummaryError;

    fn task_disappeared(&self, _task_id: &str) -> Self::Error {
        AthleteSummaryError::Repository(
            "athlete summary task disappeared before completion".to_string(),
        )
    }

    fn task_timed_out(&self, _task_id: &str) -> Self::Error {
        AthleteSummaryError::Repository("athlete summary task timed out".to_string())
    }

    fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
        parse_required_json_value(
            task.checkpoint.clone(),
            "completed athlete summary task missing persisted checkpoint",
            "invalid completed athlete summary checkpoint",
            AthleteSummaryError::Repository,
        )
    }

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok(parse_failed_or_error_message(
            parse_failed_checkpoint(task)?,
            task.error_message.clone(),
            "athlete summary task failed without an error message",
            AthleteSummaryError::Repository,
        ))
    }

    fn finish(&self, _: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        let user_id = self.user_id.clone();
        Box::pin(async move {
            base.get_summary_state(&user_id)
                .await?
                .summary
                .ok_or_else(|| {
                    AthleteSummaryError::Repository(
                        "completed athlete summary task missing persisted summary".to_string(),
                    )
                })
        })
    }
}

pub struct SchedulerBackedAthleteSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    base: Arc<Base>,
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    ids: Ids,
}

impl<Base, Tasks, Workers, Time, Ids> Clone
    for SchedulerBackedAthleteSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: Clone,
{
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            scheduler: self.scheduler.clone(),
            ids: self.ids.clone(),
        }
    }
}

impl<Base, Tasks, Workers, Time, Ids>
    SchedulerBackedAthleteSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Base: AthleteSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        base: Arc<Base>,
        scheduler: TaskSchedulerService<Tasks, Workers, Time>,
        ids: Ids,
    ) -> Self {
        Self {
            base,
            scheduler,
            ids,
        }
    }

    fn build_generate_task(
        &self,
        user_id: &str,
        force: bool,
    ) -> Result<ScheduledTask, AthleteSummaryError> {
        let task_id = self.ids.new_id("task");
        let dedupe_key = if force {
            build_force_dedupe_key(user_id, &task_id)
        } else {
            build_non_force_dedupe_key(user_id, self.current_refresh_window_start())
        };

        build_scheduled_task(NewScheduledTaskInput {
            id: task_id,
            user_id: user_id.to_string(),
            task_type: ATHLETE_SUMMARY_GENERATE_TASK_TYPE,
            payload: AthleteSummaryTaskPayload {
                user_id: user_id.to_string(),
                force,
            },
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: ATHLETE_SUMMARY_RETRY_MAX_ATTEMPTS,
                delay_seconds: ATHLETE_SUMMARY_RETRY_DELAY_SECONDS,
            },
            dedupe_key,
            execution_timeout_seconds: ATHLETE_SUMMARY_EXECUTION_TIMEOUT_SECONDS,
            leader_only: false,
            now_epoch_seconds: self.scheduler.now_epoch_seconds(),
        })
        .map_err(map_build_scheduled_task_error)
    }

    fn current_refresh_window_start(&self) -> i64 {
        current_week_monday_epoch_seconds(self.scheduler.now_epoch_seconds())
    }

    async fn wait_for_generated_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> Result<AthleteSummary, AthleteSummaryError> {
        let task = self.build_generate_task(user_id, force)?;
        self.scheduler
            .enqueue_result_task(
                task,
                map_task_scheduler_error,
                AthleteSummaryTaskResultHandler {
                    base: self.base.clone(),
                    user_id: user_id.to_string(),
                },
            )
            .await
    }
}

fn map_build_scheduled_task_error(error: BuildScheduledTaskError) -> AthleteSummaryError {
    match error {
        BuildScheduledTaskError::SerializePayload(error) => AthleteSummaryError::Repository(
            format!("failed to serialize athlete summary task payload: {error}"),
        ),
        BuildScheduledTaskError::Scheduler(error) => map_task_scheduler_error(error),
    }
}

impl<Base, Tasks, Workers, Time, Ids> AthleteSummaryUseCases
    for SchedulerBackedAthleteSummaryService<Base, Tasks, Workers, Time, Ids>
where
    Base: AthleteSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
        self.base.get_summary_state(user_id)
    }

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            if !force {
                let state = service.base.get_summary_state(&user_id).await?;
                if let Some(summary) = state.summary {
                    if !state.stale {
                        return Ok(summary);
                    }
                }
            }

            service.wait_for_generated_summary(&user_id, force).await
        })
    }

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        self.generate_summary(user_id, false)
    }

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let state = service.base.get_summary_state(&user_id).await?;
            if let Some(summary) = state.summary {
                if !state.stale {
                    return Ok(EnsuredAthleteSummary {
                        summary,
                        was_regenerated: false,
                    });
                }
            }

            let summary = service.wait_for_generated_summary(&user_id, false).await?;
            Ok(EnsuredAthleteSummary {
                summary,
                was_regenerated: true,
            })
        })
    }
}
