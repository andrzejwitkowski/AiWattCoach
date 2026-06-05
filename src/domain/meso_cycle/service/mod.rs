mod scheduler;
mod window_port;

pub use scheduler::{
    meso_cycle_generate_task_handler, MesoCycleServiceExecutor, SchedulerBackedMesoCycleService,
};
pub use window_port::TrainingPlanBackedMesoWindowPort;

use chrono::{TimeZone, Utc};

use crate::domain::{
    ai_workflow::{AttemptRecord, WorkflowPhase, WorkflowStatus},
    identity::Clock,
    intervals::serialize_planned_workout,
};

use super::{
    parse_meso_plan_window,
    ports::{
        MesoCycleGenerationOperationRepository, MesoCycleGenerator, MesoCycleProjectionRepository,
        MesoCycleToolLoopCheckpoint, MesoCycleWindowPort,
    },
    MesoCycleCalendarDay, MesoCycleError, MesoCycleFailureState, MesoCycleGenerationClaimResult,
    MesoCycleGenerationOperation, MesoCycleOverlapStatus, MesoCyclePhaseOutput,
    MesoCycleProjectedDay, MesoCycleStatus, MesoCycleWindow,
};

pub const MESO_CYCLE_STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;
pub const GENERATION_ALREADY_PENDING_MESSAGE: &str = "meso cycle generation is already pending";

pub trait MesoCycleUseCases: Send + Sync + 'static {
    fn get_status(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<MesoCycleStatus, MesoCycleError>>;

    fn list_calendar_days(
        &self,
        user_id: &str,
        from: &str,
        to: &str,
    ) -> super::BoxFuture<Result<Vec<MesoCycleCalendarDay>, MesoCycleError>>;

    fn generate_plan(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>>;

    fn get_operation(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>>;
}

#[derive(Clone)]
pub struct MesoCycleService<Ops, Projections, Generator, Window, Time>
where
    Ops: MesoCycleGenerationOperationRepository + Clone,
    Projections: MesoCycleProjectionRepository + Clone,
    Generator: MesoCycleGenerator + Clone,
    Window: MesoCycleWindowPort + Clone,
    Time: Clock + Clone,
{
    operations: Ops,
    projections: Projections,
    generator: Generator,
    window_port: Window,
    clock: Time,
}

impl<Ops, Projections, Generator, Window, Time>
    MesoCycleService<Ops, Projections, Generator, Window, Time>
where
    Ops: MesoCycleGenerationOperationRepository + Clone,
    Projections: MesoCycleProjectionRepository + Clone,
    Generator: MesoCycleGenerator + Clone,
    Window: MesoCycleWindowPort + Clone,
    Time: Clock + Clone,
{
    pub fn new(
        operations: Ops,
        projections: Projections,
        generator: Generator,
        window_port: Window,
        clock: Time,
    ) -> Self {
        Self {
            operations,
            projections,
            generator,
            window_port,
            clock,
        }
    }

    pub(crate) fn today_string(&self) -> String {
        Utc.timestamp_opt(self.clock.now_epoch_seconds(), 0)
            .single()
            .map(|now| now.date_naive().format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "1970-01-01".to_string())
    }

    fn stale_pending_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - MESO_CYCLE_STALE_PENDING_TIMEOUT_SECONDS
    }

    pub(crate) async fn resolve_window(
        &self,
        user_id: &str,
    ) -> Result<MesoCycleWindow, MesoCycleError> {
        self.window_port
            .resolve_window(user_id, &self.today_string())
            .await
    }

    pub async fn start_generation(
        &self,
        user_id: &str,
    ) -> Result<MesoCycleGenerationOperation, MesoCycleError> {
        let now = self.clock.now_epoch_seconds();
        let operation_key = MesoCycleGenerationOperation::stable_operation_key(user_id);
        let pending =
            MesoCycleGenerationOperation::pending(operation_key, user_id.to_string(), now, now);

        let operation = match self
            .operations
            .claim_pending(pending, self.stale_pending_before_epoch_seconds())
            .await?
        {
            MesoCycleGenerationClaimResult::Claimed(operation) => operation,
            MesoCycleGenerationClaimResult::AlreadyPending => {
                return Err(MesoCycleError::AlreadyPending);
            }
            MesoCycleGenerationClaimResult::AlreadyCompleted(_) => {
                return Err(MesoCycleError::AlreadyPending);
            }
        };

        let window = self.resolve_window(user_id).await?;
        let mut operation = operation;
        operation.meso_start = Some(window.meso_start);
        operation.meso_end = Some(window.meso_end);
        self.operations.upsert(operation).await
    }

    pub(crate) async fn abort_enqueue_failure(
        &self,
        operation_key: &str,
        message: String,
    ) -> Result<(), MesoCycleError> {
        let now = self.clock.now_epoch_seconds();
        let operation = self
            .operations
            .find_by_operation_key(operation_key)
            .await?
            .ok_or_else(|| {
                MesoCycleError::Repository(format!("meso operation not found: {operation_key}"))
            })?;
        let failed = self.fail_operation(&operation, message, now);
        self.operations.upsert(failed).await?;
        Ok(())
    }

    fn window_from_operation(
        operation: &MesoCycleGenerationOperation,
    ) -> Result<MesoCycleWindow, MesoCycleError> {
        let meso_start = operation.meso_start.clone().ok_or_else(|| {
            MesoCycleError::Validation("meso operation is missing meso_start".to_string())
        })?;
        let meso_end = operation.meso_end.clone().ok_or_else(|| {
            MesoCycleError::Validation("meso operation is missing meso_end".to_string())
        })?;
        Ok(MesoCycleWindow {
            meso_start,
            meso_end,
            ai_coach_last_date: None,
            source_training_plan_operation_key: None,
        })
    }

    pub(crate) async fn execute_generation(
        &self,
        operation_key: &str,
    ) -> Result<MesoCycleGenerationOperation, MesoCycleError> {
        let now = self.clock.now_epoch_seconds();
        let operation = self
            .operations
            .find_by_operation_key(operation_key)
            .await?
            .ok_or_else(|| {
                MesoCycleError::Repository(format!("meso operation not found: {operation_key}"))
            })?;

        if matches!(operation.status, WorkflowStatus::Completed) {
            return Ok(operation);
        }

        if matches!(operation.status, WorkflowStatus::Failed) {
            return Err(MesoCycleError::Unavailable(
                "meso operation failed; start a new generation".to_string(),
            ));
        }

        if !matches!(operation.status, WorkflowStatus::Pending) {
            return Err(MesoCycleError::Unavailable(format!(
                "meso operation is not runnable: {}",
                operation.operation_key
            )));
        }

        let user_id = operation.user_id.clone();
        let window = Self::window_from_operation(&operation)?;

        if operation.projection_persisted_at_epoch_seconds.is_some() {
            let mut completed = operation;
            completed.status = WorkflowStatus::Completed;
            completed.updated_at_epoch_seconds = now;
            completed.last_attempt_at_epoch_seconds = now;
            return self.operations.upsert(completed).await;
        }

        if let Some(raw_response) = operation.raw_plan_response.clone() {
            let output = MesoCyclePhaseOutput {
                raw_response,
                description: operation.raw_plan_description.clone(),
                tool_loop_state: operation.tool_loop_state.clone().unwrap_or_default(),
            };
            return self
                .finalize_generation(operation, window, output, now)
                .await;
        }

        let checkpoint_ops = self.operations.clone();
        let checkpoint_key = operation.operation_key.clone();
        let checkpoint_clock = self.clock.clone();
        let checkpoint: MesoCycleToolLoopCheckpoint = std::sync::Arc::new(move |state| {
            let ops = checkpoint_ops.clone();
            let key = checkpoint_key.clone();
            let clock = checkpoint_clock.clone();
            Box::pin(async move {
                let existing = ops.find_by_operation_key(&key).await?.ok_or_else(|| {
                    MesoCycleError::Repository("meso operation missing".to_string())
                })?;
                let updated = existing.with_tool_loop_state(state, clock.now_epoch_seconds());
                ops.upsert(updated).await?;
                Ok(())
            })
        });

        let restored_state = operation.tool_loop_state.clone();
        let output = match self
            .generator
            .generate_plan_window_with_state(&user_id, &window, restored_state, Some(checkpoint))
            .await
        {
            Ok(output) => output,
            Err(error) => {
                let failed = self.fail_operation(&operation, error.to_string(), now);
                self.operations.upsert(failed).await?;
                return Err(error);
            }
        };

        let mut persisted = operation;
        persisted.raw_plan_response = Some(output.raw_response.clone());
        persisted.raw_plan_description = output.description.clone();
        persisted.tool_loop_state = Some(output.tool_loop_state.clone());
        persisted.updated_at_epoch_seconds = now;
        persisted.last_attempt_at_epoch_seconds = now;
        let persisted = self.operations.upsert(persisted).await?;

        self.finalize_generation(persisted, window, output, now)
            .await
    }

    async fn finalize_generation(
        &self,
        operation: MesoCycleGenerationOperation,
        window: MesoCycleWindow,
        output: MesoCyclePhaseOutput,
        now: i64,
    ) -> Result<MesoCycleGenerationOperation, MesoCycleError> {
        let days = match parse_meso_plan_window(
            &output.raw_response,
            &window.meso_start,
            &window.meso_end,
        ) {
            Ok(days) => days,
            Err(error) => {
                let failed = self.fail_operation(&operation, error.to_string(), now);
                self.operations.upsert(failed).await?;
                return Err(error);
            }
        };

        let projected_days = days
            .into_iter()
            .map(|day| MesoCycleProjectedDay {
                user_id: operation.user_id.clone(),
                operation_key: operation.operation_key.clone(),
                date: day.date,
                rest_day: day.rest_day,
                rest_day_reason: day.rest_day_reason,
                workout: day.workout,
                superseded_at_epoch_seconds: None,
                created_at_epoch_seconds: now,
                updated_at_epoch_seconds: now,
            })
            .collect::<Vec<_>>();

        if let Err(error) = self
            .projections
            .replace_window(
                &operation.user_id,
                &operation.operation_key,
                projected_days,
                now,
            )
            .await
        {
            let failed = self.fail_operation(&operation, error.to_string(), now);
            self.operations.upsert(failed).await?;
            return Err(error);
        }

        let mut projected = operation;
        projected.projection_persisted_at_epoch_seconds = Some(now);
        projected.updated_at_epoch_seconds = now;
        projected.last_attempt_at_epoch_seconds = now;
        let operation = self.operations.upsert(projected).await?;

        let mut completed = operation;
        completed.status = WorkflowStatus::Completed;
        completed.raw_plan_response = Some(output.raw_response);
        completed.raw_plan_description = output.description;
        completed.tool_loop_state = Some(output.tool_loop_state);
        completed.projection_persisted_at_epoch_seconds = Some(now);
        completed.meso_start = Some(window.meso_start);
        completed.meso_end = Some(window.meso_end);
        completed.updated_at_epoch_seconds = now;
        completed.attempts.push(AttemptRecord {
            phase: WorkflowPhase::InitialGeneration,
            attempt_number: 1,
            recorded_at_epoch_seconds: now,
        });
        self.operations.upsert(completed).await
    }

    fn fail_operation(
        &self,
        operation: &MesoCycleGenerationOperation,
        message: String,
        now: i64,
    ) -> MesoCycleGenerationOperation {
        MesoCycleGenerationOperation {
            status: WorkflowStatus::Failed,
            failure: Some(MesoCycleFailureState { message }),
            updated_at_epoch_seconds: now,
            last_attempt_at_epoch_seconds: now,
            ..operation.clone()
        }
    }

    fn validate_calendar_date(value: &str) -> Result<(), MesoCycleError> {
        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
            MesoCycleError::Validation(format!("invalid calendar date {value}: {error}"))
        })?;
        Ok(())
    }

    fn projected_workout_name(day: &MesoCycleProjectedDay) -> Option<String> {
        if day.rest_day {
            return Some("Rest Day".to_string());
        }

        day.workout.as_ref().and_then(|workout| {
            workout
                .lines
                .iter()
                .find_map(|line| line.text().map(ToString::to_string))
        })
    }
}

impl<Ops, Projections, Generator, Window, Time> MesoCycleUseCases
    for MesoCycleService<Ops, Projections, Generator, Window, Time>
where
    Ops: MesoCycleGenerationOperationRepository + Clone + 'static,
    Projections: MesoCycleProjectionRepository + Clone + 'static,
    Generator: MesoCycleGenerator + Clone + 'static,
    Window: MesoCycleWindowPort + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn get_status(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<MesoCycleStatus, MesoCycleError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let latest_operation = service.operations.find_latest_by_user_id(&user_id).await?;
            let pending = service
                .operations
                .find_pending_by_user_id(&user_id)
                .await?
                .is_some();
            let window = Some(service.resolve_window(&user_id).await?);
            Ok(MesoCycleStatus {
                latest_operation,
                window,
                has_pending_generation: pending,
            })
        })
    }

    fn list_calendar_days(
        &self,
        user_id: &str,
        from: &str,
        to: &str,
    ) -> super::BoxFuture<Result<Vec<MesoCycleCalendarDay>, MesoCycleError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let from = from.to_string();
        let to = to.to_string();
        Box::pin(async move {
            let projected = service.projections.list_active_by_user_id(&user_id).await?;
            let ai_dates = service.window_port.ai_coach_active_dates(&user_id).await?;

            if from > to {
                return Err(MesoCycleError::Validation(
                    "calendar query from must be on or before to".to_string(),
                ));
            }

            Self::validate_calendar_date(&from)?;
            Self::validate_calendar_date(&to)?;

            Ok(projected
                .into_iter()
                .filter(|day| day.date >= from && day.date <= to)
                .map(|day| {
                    let overlap_status = if ai_dates.contains(&day.date) {
                        MesoCycleOverlapStatus::Outdated
                    } else {
                        MesoCycleOverlapStatus::Active
                    };
                    let name = Self::projected_workout_name(&day);
                    let raw_workout_doc = day.workout.as_ref().map(serialize_planned_workout);
                    MesoCycleCalendarDay {
                        date: day.date,
                        rest_day: day.rest_day,
                        rest_day_reason: day.rest_day_reason,
                        name,
                        raw_workout_doc,
                        overlap_status,
                    }
                })
                .collect())
        })
    }

    fn generate_plan(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.start_generation(&user_id).await })
    }

    fn get_operation(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> super::BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            service
                .operations
                .find_by_operation_key_for_user(&operation_key, &user_id)
                .await?
                .ok_or_else(|| {
                    MesoCycleError::Validation(format!("meso operation not found: {operation_key}"))
                })
        })
    }
}
