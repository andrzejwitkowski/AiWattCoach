use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    identity::{Clock, IdGenerator},
    llm::LLM_REQUEST_TIMEOUT_SECONDS,
    task_scheduler::{
        BoxFuture as TaskSchedulerBoxFuture, NewTask, ResultTaskHandler, RetryStrategy,
        ScheduledTask, SharedTaskHandler, TaskHandler, TaskRepository, TaskRunOutcome,
        TaskSchedulerError, TaskSchedulerService, TaskWorkerRepository,
    },
    workout_summary::WorkoutRecap,
};

use super::{
    BoxFuture, GeneratedTrainingPlan, TrainingPlanError, TrainingPlanGenerationOperationRepository,
    TrainingPlanGenerationService, TrainingPlanGenerator, TrainingPlanProjectionRepository,
    TrainingPlanSnapshotRepository, TrainingPlanUseCases, TrainingPlanWorkoutSummaryPort,
};

pub(crate) const TRAINING_PLAN_GENERATE_TASK_TYPE: &str =
    "training_plan.generate_for_saved_workout";
pub(crate) const TRAINING_PLAN_EXECUTION_TIMEOUT_BUFFER_SECONDS: i64 = 30;
pub(crate) const TRAINING_PLAN_EXECUTION_TIMEOUT_SECONDS: i64 =
    (LLM_REQUEST_TIMEOUT_SECONDS as i64 * 4) + TRAINING_PLAN_EXECUTION_TIMEOUT_BUFFER_SECONDS;

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

trait TrainingPlanSchedulerExecutor: TrainingPlanUseCases + Send + Sync + 'static {
    fn operation_key_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> String;

    fn load_generated_plan_from_state(
        &self,
        operation_key: &str,
    ) -> TaskSchedulerBoxFuture<Result<Option<GeneratedTrainingPlan>, TrainingPlanError>>;
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time, Refresh>
    TrainingPlanSchedulerExecutor
    for TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    fn operation_key_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> String {
        self.operation_key(user_id, workout_id, saved_at_epoch_seconds)
    }

    fn load_generated_plan_from_state(
        &self,
        operation_key: &str,
    ) -> TaskSchedulerBoxFuture<Result<Option<GeneratedTrainingPlan>, TrainingPlanError>> {
        let service = self.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            service
                .existing_generated_plan_with_healed_operation(&operation_key)
                .await
        })
    }
}

fn parse_task_payload(task: &ScheduledTask) -> Result<TrainingPlanTaskPayload, TrainingPlanError> {
    serde_json::from_value(task.payload.clone()).map_err(|error| {
        TrainingPlanError::Repository(format!(
            "invalid training plan generate task payload: {error}"
        ))
    })
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

fn parse_completed_checkpoint(
    task: &ScheduledTask,
) -> Result<Option<CompletedTrainingPlanTaskCheckpoint>, TrainingPlanError> {
    task.checkpoint
        .clone()
        .map(|value| {
            serde_json::from_value(value).map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "invalid completed training plan checkpoint: {error}"
                ))
            })
        })
        .transpose()
}

fn parse_failed_checkpoint(
    task: &ScheduledTask,
) -> Result<Option<TrainingPlanError>, TrainingPlanError> {
    task.checkpoint
        .clone()
        .map(|value| {
            serde_json::from_value::<SerializedTrainingPlanError>(value)
                .map(deserialize_training_plan_error)
                .map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "invalid failed training plan checkpoint: {error}"
                    ))
                })
        })
        .transpose()
}

fn build_completed_checkpoint(
    generated: &GeneratedTrainingPlan,
) -> Result<serde_json::Value, TrainingPlanError> {
    serde_json::to_value(CompletedTrainingPlanTaskCheckpoint {
        operation_key: generated.snapshot.operation_key.clone(),
        was_generated: generated.was_generated,
    })
    .map_err(|error| {
        TrainingPlanError::Repository(format!(
            "failed to serialize completed training plan checkpoint: {error}"
        ))
    })
}

struct TrainingPlanGenerateTaskHandler<Base> {
    base: Arc<Base>,
}

impl<Base> TaskHandler for TrainingPlanGenerateTaskHandler<Base>
where
    Base: TrainingPlanSchedulerExecutor,
{
    fn task_type(&self) -> &'static str {
        TRAINING_PLAN_GENERATE_TASK_TYPE
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let base = self.base.clone();
        Box::pin(async move {
            let payload = match parse_task_payload(&task) {
                Ok(payload) => payload,
                Err(error) => {
                    return TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: error.to_string(),
                        retryable: false,
                        retry_delay_seconds: None,
                    };
                }
            };

            match base
                .generate_for_saved_workout(
                    &payload.user_id,
                    &payload.workout_id,
                    payload.saved_at_epoch_seconds,
                )
                .await
            {
                Ok(generated) => match build_completed_checkpoint(&generated) {
                    Ok(checkpoint) => TaskRunOutcome::Completed {
                        checkpoint: Some(checkpoint),
                    },
                    Err(error) => TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: error.to_string(),
                        retryable: false,
                        retry_delay_seconds: None,
                    },
                },
                Err(error) => TaskRunOutcome::Failed {
                    checkpoint: serde_json::to_value(serialize_training_plan_error(&error)).ok(),
                    error_message: error.to_string(),
                    retryable: false,
                    retry_delay_seconds: None,
                },
            }
        })
    }
}

#[allow(private_bounds)]
pub fn training_plan_generate_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: TrainingPlanSchedulerExecutor,
{
    Arc::new(TrainingPlanGenerateTaskHandler { base })
}

#[derive(Clone)]
struct TrainingPlanTaskResultHandler<Base> {
    base: Arc<Base>,
}

impl<Base> ResultTaskHandler for TrainingPlanTaskResultHandler<Base>
where
    Base: TrainingPlanSchedulerExecutor,
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
        parse_completed_checkpoint(task)?.ok_or_else(|| {
            TrainingPlanError::Repository(
                "completed training plan task missing persisted checkpoint".to_string(),
            )
        })
    }

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
        Ok(parse_failed_checkpoint(task)?.unwrap_or_else(|| {
            task.error_message
                .clone()
                .map(TrainingPlanError::Repository)
                .unwrap_or_else(|| {
                    TrainingPlanError::Repository(
                        "training plan task failed without an error message".to_string(),
                    )
                })
        }))
    }

    fn finish(&self, completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move {
            let generated = base
                .load_generated_plan_from_state(&completed.operation_key)
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

impl<Base, Tasks, Workers, Time, Ids>
    SchedulerBackedTrainingPlanService<Base, Tasks, Workers, Time, Ids>
where
    Base: TrainingPlanUseCases + 'static,
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
}

#[allow(private_bounds)]
impl<Base, Tasks, Workers, Time, Ids>
    SchedulerBackedTrainingPlanService<Base, Tasks, Workers, Time, Ids>
where
    Base: TrainingPlanSchedulerExecutor,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn build_generate_task(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> Result<ScheduledTask, TrainingPlanError> {
        let dedupe_key =
            self.base
                .operation_key_for_saved_workout(user_id, workout_id, saved_at_epoch_seconds);
        ScheduledTask::new(
            NewTask {
                id: self.ids.new_id("task"),
                user_id: user_id.to_string(),
                task_type: TRAINING_PLAN_GENERATE_TASK_TYPE.to_string(),
                payload: serde_json::to_value(TrainingPlanTaskPayload {
                    user_id: user_id.to_string(),
                    workout_id: workout_id.to_string(),
                    saved_at_epoch_seconds,
                })
                .map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "failed to serialize training plan task payload: {error}"
                    ))
                })?,
                retry_strategy: RetryStrategy::Never,
                dedupe_key,
                execution_timeout_seconds: TRAINING_PLAN_EXECUTION_TIMEOUT_SECONDS,
                leader_only: false,
            },
            self.scheduler.now_epoch_seconds(),
        )
        .map_err(map_task_scheduler_error)
    }

    async fn existing_generated_plan(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> Result<Option<GeneratedTrainingPlan>, TrainingPlanError> {
        let operation_key =
            self.base
                .operation_key_for_saved_workout(user_id, workout_id, saved_at_epoch_seconds);
        self.base
            .load_generated_plan_from_state(&operation_key)
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

#[allow(private_bounds)]
impl<Base, Tasks, Workers, Time, Ids> TrainingPlanUseCases
    for SchedulerBackedTrainingPlanService<Base, Tasks, Workers, Time, Ids>
where
    Base: TrainingPlanSchedulerExecutor,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
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
