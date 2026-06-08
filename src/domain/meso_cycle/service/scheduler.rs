use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::LLM_REQUEST_TIMEOUT_SECONDS,
    task_scheduler::{
        build_scheduled_task, scheduled_task_handler, BuildScheduledTaskError,
        NewScheduledTaskInput, RetryStrategy, ScheduledTask, ScheduledTaskExecutor,
        SharedTaskHandler, TaskRepository, TaskRunOutcome, TaskSchedulerError,
        TaskSchedulerService, TaskWorkerRepository,
    },
};

use super::{
    MesoCycleError, MesoCycleGenerationOperation, MesoCycleService, MesoCycleUseCases,
    MESO_CYCLE_STALE_PENDING_TIMEOUT_SECONDS,
};

pub(crate) const MESO_CYCLE_GENERATE_TASK_TYPE: &str = "meso_cycle.generate";
pub(crate) const MESO_CYCLE_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const MESO_CYCLE_EXECUTION_TIMEOUT_SECONDS: i64 =
    (LLM_REQUEST_TIMEOUT_SECONDS as i64 * 4) + MESO_CYCLE_EXECUTION_TIMEOUT_BUFFER_SECONDS;
pub(crate) const MESO_CYCLE_RETRY_MAX_ATTEMPTS: u32 = 3;
pub(crate) const MESO_CYCLE_RETRY_DELAY_SECONDS: i64 = MESO_CYCLE_STALE_PENDING_TIMEOUT_SECONDS;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MesoCycleTaskPayload {
    user_id: String,
    operation_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SerializedMesoCycleError {
    Unavailable { message: String },
    Repository { message: String },
    Validation { message: String },
    AlreadyPending,
    NotConfigured,
}

fn map_task_scheduler_error(error: TaskSchedulerError) -> MesoCycleError {
    match error {
        TaskSchedulerError::Validation(message) | TaskSchedulerError::Conflict(message) => {
            MesoCycleError::Validation(message)
        }
        TaskSchedulerError::Repository(message) => MesoCycleError::Repository(message),
    }
}

fn serialize_meso_cycle_error(error: &MesoCycleError) -> SerializedMesoCycleError {
    match error {
        MesoCycleError::Unavailable(message) => SerializedMesoCycleError::Unavailable {
            message: message.clone(),
        },
        MesoCycleError::Repository(message) => SerializedMesoCycleError::Repository {
            message: message.clone(),
        },
        MesoCycleError::Validation(message) => SerializedMesoCycleError::Validation {
            message: message.clone(),
        },
        MesoCycleError::AlreadyPending => SerializedMesoCycleError::AlreadyPending,
        MesoCycleError::NotConfigured => SerializedMesoCycleError::NotConfigured,
    }
}

struct MesoCycleGenerateTaskExecutor<Base> {
    base: Arc<Base>,
}

impl<Base> MesoCycleGenerateTaskExecutor<Base>
where
    Base: MesoCycleServiceExecutor + 'static,
{
    fn map_task_failure(error: MesoCycleError) -> TaskRunOutcome {
        let (retryable, retry_delay_seconds) = match &error {
            MesoCycleError::Repository(_) => (true, None),
            MesoCycleError::Unavailable(message)
                if message.contains("not runnable") || message.contains("failed; start") =>
            {
                (false, None)
            }
            MesoCycleError::Unavailable(_) => (true, None),
            MesoCycleError::AlreadyPending
            | MesoCycleError::NotConfigured
            | MesoCycleError::Validation(_) => (false, None),
        };

        TaskRunOutcome::Failed {
            checkpoint: serde_json::to_value(serialize_meso_cycle_error(&error)).ok(),
            error_message: error.to_string(),
            retryable,
            retry_delay_seconds,
        }
    }
}

impl<Base> ScheduledTaskExecutor for MesoCycleGenerateTaskExecutor<Base>
where
    Base: MesoCycleServiceExecutor + 'static,
{
    type Payload = MesoCycleTaskPayload;
    type Output = MesoCycleGenerationOperation;
    type Error = MesoCycleError;

    fn task_type(&self) -> &'static str {
        MESO_CYCLE_GENERATE_TASK_TYPE
    }

    fn parse_error(&self, error: serde_json::Error) -> Self::Error {
        MesoCycleError::Repository(format!("invalid meso cycle generate task payload: {error}"))
    }

    fn run(
        &self,
        _task: ScheduledTask,
        payload: Self::Payload,
    ) -> super::super::BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move { base.execute_generation(&payload.operation_key).await })
    }

    fn completed_checkpoint(
        &self,
        _task_id: &str,
        output: &Self::Output,
    ) -> Result<Option<serde_json::Value>, Self::Error> {
        Ok(Some(serde_json::json!({
            "operation_key": output.operation_key,
        })))
    }

    fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome {
        Self::map_task_failure(error)
    }
}

pub trait MesoCycleServiceExecutor: Send + Sync {
    fn execute_generation(
        &self,
        operation_key: &str,
    ) -> super::super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>>;

    fn abort_enqueue_failure(
        &self,
        operation_key: &str,
        message: String,
    ) -> super::super::BoxFuture<Result<(), MesoCycleError>>;
}

impl<Ops, Projections, Generator, Window, Time> MesoCycleServiceExecutor
    for MesoCycleService<Ops, Projections, Generator, Window, Time>
where
    Ops: super::super::ports::MesoCycleGenerationOperationRepository + Clone + 'static,
    Projections: super::super::ports::MesoCycleProjectionRepository + Clone + 'static,
    Generator: super::super::ports::MesoCycleGenerator + Clone + 'static,
    Window: super::super::ports::MesoCycleWindowPort + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn execute_generation(
        &self,
        operation_key: &str,
    ) -> super::super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        let service = self.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move { service.execute_generation(&operation_key).await })
    }

    fn abort_enqueue_failure(
        &self,
        operation_key: &str,
        message: String,
    ) -> super::super::BoxFuture<Result<(), MesoCycleError>> {
        let service = self.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move { service.abort_enqueue_failure(&operation_key, message).await })
    }
}

pub fn meso_cycle_generate_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: MesoCycleServiceExecutor + 'static,
{
    Arc::new(scheduled_task_handler(MesoCycleGenerateTaskExecutor {
        base,
    }))
}

pub struct SchedulerBackedMesoCycleService<Base, Tasks, Workers, Time, Ids>
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
    for SchedulerBackedMesoCycleService<Base, Tasks, Workers, Time, Ids>
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
    SchedulerBackedMesoCycleService<Base, Tasks, Workers, Time, Ids>
where
    Base: MesoCycleUseCases + MesoCycleServiceExecutor + 'static,
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
        operation_key: &str,
        requested_at_epoch_seconds: i64,
    ) -> Result<ScheduledTask, MesoCycleError> {
        build_scheduled_task(NewScheduledTaskInput {
            id: self.ids.new_id("task"),
            user_id: user_id.to_string(),
            task_type: MESO_CYCLE_GENERATE_TASK_TYPE,
            payload: MesoCycleTaskPayload {
                user_id: user_id.to_string(),
                operation_key: operation_key.to_string(),
            },
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: MESO_CYCLE_RETRY_MAX_ATTEMPTS,
                delay_seconds: MESO_CYCLE_RETRY_DELAY_SECONDS,
            },
            dedupe_key: format!("meso-cycle:generate:{user_id}:{requested_at_epoch_seconds}"),
            execution_timeout_seconds: MESO_CYCLE_EXECUTION_TIMEOUT_SECONDS,
            leader_only: false,
            now_epoch_seconds: self.scheduler.now_epoch_seconds(),
        })
        .map_err(map_build_scheduled_task_error)
    }

    async fn enqueue_generation(
        &self,
        user_id: &str,
    ) -> Result<MesoCycleGenerationOperation, MesoCycleError> {
        let operation = self.base.generate_plan(user_id).await?;
        let task = self.build_generate_task(
            user_id,
            &operation.operation_key,
            operation.requested_at_epoch_seconds,
        )?;
        if let Err(error) = self.scheduler.enqueue_no_result_task(task).await {
            let mapped = map_task_scheduler_error(error);
            let _ = self
                .base
                .abort_enqueue_failure(&operation.operation_key, mapped.to_string())
                .await;
            return Err(mapped);
        }
        Ok(operation)
    }
}

fn map_build_scheduled_task_error(error: BuildScheduledTaskError) -> MesoCycleError {
    match error {
        BuildScheduledTaskError::SerializePayload(error) => MesoCycleError::Repository(format!(
            "failed to serialize meso cycle task payload: {error}"
        )),
        BuildScheduledTaskError::Scheduler(error) => map_task_scheduler_error(error),
    }
}

impl<Base, Tasks, Workers, Time, Ids> MesoCycleUseCases
    for SchedulerBackedMesoCycleService<Base, Tasks, Workers, Time, Ids>
where
    Base: MesoCycleUseCases + MesoCycleServiceExecutor + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn get_status(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<super::super::MesoCycleStatus, MesoCycleError>> {
        self.base.get_status(user_id)
    }

    fn list_calendar_days(
        &self,
        user_id: &str,
        from: &str,
        to: &str,
    ) -> super::super::BoxFuture<Result<Vec<super::super::MesoCycleCalendarDay>, MesoCycleError>>
    {
        self.base.list_calendar_days(user_id, from, to)
    }

    fn generate_plan(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.enqueue_generation(&user_id).await })
    }

    fn get_operation(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> super::super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        self.base.get_operation(user_id, operation_key)
    }
}
