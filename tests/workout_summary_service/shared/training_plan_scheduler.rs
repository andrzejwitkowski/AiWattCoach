use std::sync::{Arc, Mutex};
use std::time::Duration;

use aiwattcoach::{
    config::spawn_task_worker,
    domain::{
        ai_workflow::ValidationIssue,
        task_scheduler::{TaskSchedulerService, TaskWorkerConfig},
        training_plan::{
            training_plan_generate_task_handler, SchedulerBackedTrainingPlanService,
            TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
            TrainingPlanGenerationOperationRepository, TrainingPlanGenerationService,
            TrainingPlanGenerator, TrainingPlanPhaseOutput, TrainingPlanPlanningContext,
            TrainingPlanProjectedDay, TrainingPlanProjectionRepository, TrainingPlanSnapshot,
            TrainingPlanSnapshotRepository, TrainingPlanUseCases,
        },
        workout_summary::WorkoutRecap,
    },
    BackgroundTaskHandle,
};

use super::{test_service, InMemoryWorkoutSummaryRepository, TestIdGenerator};
use crate::{
    task_scheduler_clock_support::TestClock as SchedulerClock,
    task_scheduler_repository_support::{InMemoryTaskRepository, InMemoryTaskWorkerRepository},
};

pub(crate) struct SchedulerBackedTrainingPlanHarness {
    pub(crate) service: Arc<dyn TrainingPlanUseCases>,
    pub(crate) worker: BackgroundTaskHandle,
}

pub(crate) fn scheduler_backed_training_plan_service(
    training_plan_repo: InMemoryWorkoutSummaryRepository,
) -> SchedulerBackedTrainingPlanHarness {
    let snapshot_store = Arc::new(Mutex::new(None));
    let direct_training_plan = Arc::new(TrainingPlanGenerationService::new(
        SaveFlowSnapshotRepository::new(snapshot_store.clone()),
        SaveFlowProjectionRepository::new(snapshot_store),
        SaveFlowOperationRepository::default(),
        SaveFlowTrainingPlanGenerator,
        aiwattcoach::main_runtime::TrainingPlanWorkoutSummaryAdapter::new(Arc::new(test_service(
            training_plan_repo,
        ))),
        SchedulerClock::new(1_700_000_000),
    ));
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        SchedulerClock::new(1_700_000_000),
    );
    let worker = spawn_task_worker(
        scheduler.clone(),
        "training-plan-worker".to_string(),
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: 30,
            heartbeat_interval: Duration::from_secs(10),
            idle_poll_interval: Duration::from_millis(10),
            max_concurrency: 2,
        },
        vec![training_plan_generate_task_handler(
            direct_training_plan.clone(),
        )],
    )
    .expect("training plan test worker should spawn");

    SchedulerBackedTrainingPlanHarness {
        service: Arc::new(SchedulerBackedTrainingPlanService::new(
            direct_training_plan,
            scheduler,
            TestIdGenerator::default(),
        )),
        worker,
    }
}

#[derive(Clone, Default)]
struct SaveFlowSnapshotRepository {
    snapshot: Arc<Mutex<Option<TrainingPlanSnapshot>>>,
}

impl SaveFlowSnapshotRepository {
    fn new(snapshot: Arc<Mutex<Option<TrainingPlanSnapshot>>>) -> Self {
        Self { snapshot }
    }
}

impl TrainingPlanSnapshotRepository for SaveFlowSnapshotRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanSnapshot>, aiwattcoach::domain::training_plan::TrainingPlanError>,
    > {
        let snapshot = self
            .snapshot
            .lock()
            .unwrap()
            .clone()
            .filter(|snapshot| snapshot.operation_key == operation_key);
        Box::pin(async move { Ok(snapshot) })
    }
}

#[derive(Clone, Default)]
struct SaveFlowProjectionRepository {
    days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
    snapshot: Arc<Mutex<Option<TrainingPlanSnapshot>>>,
}

impl SaveFlowProjectionRepository {
    fn new(snapshot: Arc<Mutex<Option<TrainingPlanSnapshot>>>) -> Self {
        Self {
            days: Arc::new(Mutex::new(Vec::new())),
            snapshot,
        }
    }
}

impl TrainingPlanProjectionRepository for SaveFlowProjectionRepository {
    fn list_active_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            Vec<TrainingPlanProjectedDay>,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move { Ok(days) })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            Vec<TrainingPlanProjectedDay>,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let operation_key = operation_key.to_string();
        let days = self
            .days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.operation_key == operation_key)
            .cloned()
            .collect::<Vec<_>>();
        Box::pin(async move { Ok(days) })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            Vec<TrainingPlanProjectedDay>,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let days = self
            .days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.user_id == user_id && day.operation_key == operation_key)
            .cloned()
            .collect::<Vec<_>>();
        Box::pin(async move { Ok(days) })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            aiwattcoach::domain::training_plan::TrainingPlanReplacementResult,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let days = self.days.clone();
        let snapshot_store = self.snapshot.clone();
        Box::pin(async move {
            *days.lock().unwrap() = projected_days.clone();
            *snapshot_store.lock().unwrap() = Some(snapshot.clone());
            Ok(
                aiwattcoach::domain::training_plan::TrainingPlanReplacementResult {
                    snapshot,
                    projected_days,
                    superseded_date_range: None,
                },
            )
        })
    }

    fn update_supervisor_status(
        &self,
        user_id: &str,
        operation_key: &str,
        supervisor_status: Option<
            aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorStatus,
        >,
        updated_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<(), aiwattcoach::domain::training_plan::TrainingPlanError>,
    > {
        let days = self.days.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let mut days = days.lock().unwrap();
            for day in days.iter_mut().filter(|day| {
                day.user_id == user_id
                    && day.operation_key == operation_key
                    && day.superseded_at_epoch_seconds.is_none()
            }) {
                day.supervisor_status = supervisor_status;
                day.updated_at_epoch_seconds = updated_at_epoch_seconds;
            }
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct SaveFlowOperationRepository {
    operation: Arc<Mutex<Option<TrainingPlanGenerationOperation>>>,
}

impl TrainingPlanGenerationOperationRepository for SaveFlowOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            Option<TrainingPlanGenerationOperation>,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let operation = self
            .operation
            .lock()
            .unwrap()
            .clone()
            .filter(|operation| operation.operation_key == operation_key);
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        _stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            TrainingPlanGenerationClaimResult,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let store = self.operation.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            if let Some(existing) = store.clone() {
                return Ok(TrainingPlanGenerationClaimResult::Existing(existing));
            }
            *store = Some(operation.clone());
            Ok(TrainingPlanGenerationClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: TrainingPlanGenerationOperation,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            TrainingPlanGenerationOperation,
            aiwattcoach::domain::training_plan::TrainingPlanError,
        >,
    > {
        let store = self.operation.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(operation.clone());
            Ok(operation)
        })
    }
}

#[derive(Clone)]
struct SaveFlowTrainingPlanGenerator;

impl TrainingPlanGenerator for SaveFlowTrainingPlanGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<WorkoutRecap, aiwattcoach::domain::training_plan::TrainingPlanError>,
    > {
        Box::pin(async move {
            Ok(WorkoutRecap::generated(
                "Saved workout recap".to_string(),
                "openrouter".to_string(),
                "google/gemini-3-flash-preview".to_string(),
                saved_at_epoch_seconds,
            ))
        })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, aiwattcoach::domain::training_plan::TrainingPlanError>,
    > {
        Box::pin(async {
            Ok(TrainingPlanPhaseOutput {
                raw_response: (0..14)
                    .map(|offset| {
                        let day = 15 + offset;
                        format!("2023-11-{day:02}\nRest Day")
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                tool_loop_state: aiwattcoach::domain::llm_tools::LlmToolLoopState::default(),
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _invalid_day_sections: &str,
        _issues: Vec<ValidationIssue>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, aiwattcoach::domain::training_plan::TrainingPlanError>,
    > {
        Box::pin(async { unreachable!("save-flow test does not use corrections") })
    }
}
