use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    identity::{Clock, IdGenerator},
    llm::LLM_REQUEST_TIMEOUT_SECONDS,
    task_scheduler::{
        build_scheduled_task, parse_failed_or_error_message, parse_optional_json_value,
        parse_required_json_value, scheduled_task_handler, serialize_json_value,
        BuildScheduledTaskError, NewScheduledTaskInput, ResultTaskHandler, RetryStrategy,
        ScheduledTask, ScheduledTaskExecutor, SharedTaskHandler, TaskRepository, TaskRunOutcome,
        TaskSchedulerError, TaskSchedulerService, TaskWorkerRepository,
    },
    training_plan_supervisor::TrainingPlanSupervisorScheduler,
    workout_summary::WorkoutRecap,
};

use super::{
    BoxFuture, GeneratedTrainingPlan, TrainingPlanError, TrainingPlanGenerationOperationRepository,
    TrainingPlanGenerationService, TrainingPlanGenerator, TrainingPlanProjectionRepository,
    TrainingPlanSnapshotRepository, TrainingPlanUseCases, TrainingPlanWorkoutSummaryPort,
    TRAINING_PLAN_STALE_PENDING_TIMEOUT_SECONDS,
};

pub(crate) const TRAINING_PLAN_GENERATE_TASK_TYPE: &str =
    "training_plan.generate_for_saved_workout";
pub(crate) const TRAINING_PLAN_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const TRAINING_PLAN_EXECUTION_TIMEOUT_SECONDS: i64 =
    (LLM_REQUEST_TIMEOUT_SECONDS as i64 * 4) + TRAINING_PLAN_EXECUTION_TIMEOUT_BUFFER_SECONDS;
pub(crate) const TRAINING_PLAN_RETRY_MAX_ATTEMPTS: u32 = 3;
// Match the stale pending reclaim window so panic/restart retries do not race into
// a still-pending durable operation and fail with "already in progress".
pub(crate) const TRAINING_PLAN_RETRY_DELAY_SECONDS: i64 =
    TRAINING_PLAN_STALE_PENDING_TIMEOUT_SECONDS;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrainingPlanTaskPayload {
    user_id: String,
    workout_id: String,
    saved_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SerializedTrainingPlanError {
    Unavailable { message: String },
    Repository { message: String },
    Validation { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletedTrainingPlanTaskCheckpoint {
    operation_key: String,
    was_generated: bool,
}

fn map_task_scheduler_error(error: TaskSchedulerError) -> TrainingPlanError {
    match error {
        TaskSchedulerError::Validation(message)
        | TaskSchedulerError::Conflict(message)
        | TaskSchedulerError::Repository(message) => TrainingPlanError::Repository(message),
    }
}

fn serialize_training_plan_error(error: &TrainingPlanError) -> SerializedTrainingPlanError {
    match error {
        TrainingPlanError::Unavailable(message) => SerializedTrainingPlanError::Unavailable {
            message: message.clone(),
        },
        TrainingPlanError::Repository(message) => SerializedTrainingPlanError::Repository {
            message: message.clone(),
        },
        TrainingPlanError::Validation(message) => SerializedTrainingPlanError::Validation {
            message: message.clone(),
        },
        TrainingPlanError::GeminiSupervisorWebhookNotConfigured => {
            SerializedTrainingPlanError::Unavailable {
                message: "Gemini supervisor webhook is not configured".to_string(),
            }
        }
        TrainingPlanError::GeminiSupervisorWebhookUnauthorized => {
            SerializedTrainingPlanError::Validation {
                message: "Gemini supervisor webhook token is invalid".to_string(),
            }
        }
    }
}

fn deserialize_training_plan_error(error: SerializedTrainingPlanError) -> TrainingPlanError {
    match error {
        SerializedTrainingPlanError::Unavailable { message } => {
            TrainingPlanError::Unavailable(message)
        }
        SerializedTrainingPlanError::Repository { message } => {
            TrainingPlanError::Repository(message)
        }
        SerializedTrainingPlanError::Validation { message } => {
            TrainingPlanError::Validation(message)
        }
    }
}

fn parse_failed_checkpoint(
    task: &ScheduledTask,
) -> Result<Option<TrainingPlanError>, TrainingPlanError> {
    parse_optional_json_value::<SerializedTrainingPlanError, TrainingPlanError>(
        task.checkpoint.clone(),
        "invalid failed training plan checkpoint",
        TrainingPlanError::Repository,
    )
    .map(|error| error.map(deserialize_training_plan_error))
}

fn build_completed_checkpoint(
    generated: &GeneratedTrainingPlan,
) -> Result<serde_json::Value, TrainingPlanError> {
    serialize_json_value(
        &CompletedTrainingPlanTaskCheckpoint {
            operation_key: generated.snapshot.operation_key.clone(),
            was_generated: generated.was_generated,
        },
        "failed to serialize completed training plan checkpoint",
        TrainingPlanError::Repository,
    )
}

type DirectTrainingPlanService<
    Snapshots,
    Projections,
    Operations,
    Generator,
    WorkoutSummary,
    ServiceTime,
    Supervisor,
    Refresh,
> = TrainingPlanGenerationService<
    Snapshots,
    Projections,
    Operations,
    Generator,
    WorkoutSummary,
    ServiceTime,
    Supervisor,
    Refresh,
>;

type SharedDirectTrainingPlanService<
    Snapshots,
    Projections,
    Operations,
    Generator,
    WorkoutSummary,
    ServiceTime,
    Supervisor,
    Refresh,
> = Arc<
    DirectTrainingPlanService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
    >,
>;

struct TrainingPlanGenerateTaskExecutor<Base> {
    base: Arc<Base>,
}

impl<Base> ScheduledTaskExecutor for TrainingPlanGenerateTaskExecutor<Base>
where
    Base: TrainingPlanUseCases + 'static,
{
    type Payload = TrainingPlanTaskPayload;
    type Output = GeneratedTrainingPlan;
    type Error = TrainingPlanError;

    fn task_type(&self) -> &'static str {
        TRAINING_PLAN_GENERATE_TASK_TYPE
    }

    fn parse_error(&self, error: serde_json::Error) -> Self::Error {
        TrainingPlanError::Repository(format!(
            "invalid training plan generate task payload: {error}"
        ))
    }

    fn run(
        &self,
        _task: ScheduledTask,
        payload: Self::Payload,
    ) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move {
            base.generate_for_saved_workout(
                &payload.user_id,
                &payload.workout_id,
                payload.saved_at_epoch_seconds,
            )
            .await
        })
    }

    fn completed_checkpoint(
        &self,
        _task_id: &str,
        output: &Self::Output,
    ) -> Result<Option<serde_json::Value>, Self::Error> {
        build_completed_checkpoint(output).map(Some)
    }

    fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome {
        TaskRunOutcome::Failed {
            checkpoint: serde_json::to_value(serialize_training_plan_error(&error)).ok(),
            error_message: error.to_string(),
            retryable: false,
            retry_delay_seconds: None,
        }
    }
}

pub fn training_plan_generate_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: TrainingPlanUseCases + 'static,
{
    Arc::new(scheduled_task_handler(TrainingPlanGenerateTaskExecutor {
        base,
    }))
}

#[derive(Clone)]
struct TrainingPlanTaskResultHandler<
    Snapshots,
    Projections,
    Operations,
    Generator,
    WorkoutSummary,
    ServiceTime,
    Supervisor,
    Refresh,
> where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    ServiceTime: Clock + Clone + 'static,
    Supervisor: TrainingPlanSupervisorScheduler + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    base: SharedDirectTrainingPlanService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
    >,
}

impl<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
    > ResultTaskHandler
    for TrainingPlanTaskResultHandler<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    ServiceTime: Clock + Clone + 'static,
    Supervisor: TrainingPlanSupervisorScheduler + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    type Completed = CompletedTrainingPlanTaskCheckpoint;
    type Output = GeneratedTrainingPlan;
    type Error = TrainingPlanError;

    fn task_disappeared(&self, _task_id: &str) -> Self::Error {
        TrainingPlanError::Repository(
            "training plan task disappeared before completion".to_string(),
        )
    }

    fn task_timed_out(&self, _task_id: &str) -> Self::Error {
        TrainingPlanError::Repository("training plan task timed out".to_string())
    }

    fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
        parse_required_json_value(
            task.checkpoint.clone(),
            "completed training plan task missing persisted checkpoint",
            "invalid completed training plan checkpoint",
            TrainingPlanError::Repository,
        )
    }

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok(parse_failed_or_error_message(
            parse_failed_checkpoint(task)?,
            task.error_message.clone(),
            "training plan task failed without an error message",
            TrainingPlanError::Repository,
        ))
    }

    fn finish(&self, completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move {
            let generated = base
                .existing_generated_plan_with_healed_operation(&completed.operation_key)
                .await?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(
                        "completed training plan task missing persisted snapshot".to_string(),
                    )
                })?;
            Ok(GeneratedTrainingPlan {
                snapshot: generated.snapshot,
                active_projected_days: generated.active_projected_days,
                was_generated: completed.was_generated,
            })
        })
    }
}

pub struct SchedulerBackedTrainingPlanService<Base, Tasks, Workers, Time, Ids>
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
    for SchedulerBackedTrainingPlanService<Base, Tasks, Workers, Time, Ids>
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

impl<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
        Tasks,
        Workers,
        SchedulerTime,
        Ids,
    >
    SchedulerBackedTrainingPlanService<
        DirectTrainingPlanService<
            Snapshots,
            Projections,
            Operations,
            Generator,
            WorkoutSummary,
            ServiceTime,
            Supervisor,
            Refresh,
        >,
        Tasks,
        Workers,
        SchedulerTime,
        Ids,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    ServiceTime: Clock + Clone + 'static,
    Supervisor: TrainingPlanSupervisorScheduler + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    SchedulerTime: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        base: SharedDirectTrainingPlanService<
            Snapshots,
            Projections,
            Operations,
            Generator,
            WorkoutSummary,
            ServiceTime,
            Supervisor,
            Refresh,
        >,
        scheduler: TaskSchedulerService<Tasks, Workers, SchedulerTime>,
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
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> Result<ScheduledTask, TrainingPlanError> {
        let dedupe_key = self
            .base
            .operation_key(user_id, workout_id, saved_at_epoch_seconds);
        build_scheduled_task(NewScheduledTaskInput {
            id: self.ids.new_id("task"),
            user_id: user_id.to_string(),
            task_type: TRAINING_PLAN_GENERATE_TASK_TYPE,
            payload: TrainingPlanTaskPayload {
                user_id: user_id.to_string(),
                workout_id: workout_id.to_string(),
                saved_at_epoch_seconds,
            },
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: TRAINING_PLAN_RETRY_MAX_ATTEMPTS,
                delay_seconds: TRAINING_PLAN_RETRY_DELAY_SECONDS,
            },
            dedupe_key,
            execution_timeout_seconds: TRAINING_PLAN_EXECUTION_TIMEOUT_SECONDS,
            leader_only: false,
            now_epoch_seconds: self.scheduler.now_epoch_seconds(),
        })
        .map_err(map_build_scheduled_task_error)
    }

    async fn existing_generated_plan(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> Result<Option<GeneratedTrainingPlan>, TrainingPlanError> {
        let operation_key = self
            .base
            .operation_key(user_id, workout_id, saved_at_epoch_seconds);
        self.base
            .existing_generated_plan_with_healed_operation(&operation_key)
            .await
    }

    async fn wait_for_generated_plan(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> Result<GeneratedTrainingPlan, TrainingPlanError> {
        if let Some(existing) = self
            .existing_generated_plan(user_id, workout_id, saved_at_epoch_seconds)
            .await?
        {
            return Ok(existing);
        }

        let task = self.build_generate_task(user_id, workout_id, saved_at_epoch_seconds)?;
        self.scheduler
            .enqueue_result_task(
                task,
                map_task_scheduler_error,
                TrainingPlanTaskResultHandler {
                    base: self.base.clone(),
                },
            )
            .await
    }
}

fn map_build_scheduled_task_error(error: BuildScheduledTaskError) -> TrainingPlanError {
    match error {
        BuildScheduledTaskError::SerializePayload(error) => TrainingPlanError::Repository(format!(
            "failed to serialize training plan task payload: {error}"
        )),
        BuildScheduledTaskError::Scheduler(error) => map_task_scheduler_error(error),
    }
}

impl<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        ServiceTime,
        Supervisor,
        Refresh,
        Tasks,
        Workers,
        SchedulerTime,
        Ids,
    > TrainingPlanUseCases
    for SchedulerBackedTrainingPlanService<
        TrainingPlanGenerationService<
            Snapshots,
            Projections,
            Operations,
            Generator,
            WorkoutSummary,
            ServiceTime,
            Supervisor,
            Refresh,
        >,
        Tasks,
        Workers,
        SchedulerTime,
        Ids,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    ServiceTime: Clock + Clone + 'static,
    Supervisor: TrainingPlanSupervisorScheduler + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    SchedulerTime: Clock,
    Ids: IdGenerator,
{
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        self.base
            .generate_recap_for_saved_workout(user_id, workout_id, saved_at_epoch_seconds)
    }

    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<GeneratedTrainingPlan, TrainingPlanError>> {
        let service = (*self).clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            service
                .wait_for_generated_plan(&user_id, &workout_id, saved_at_epoch_seconds)
                .await
        })
    }
}
