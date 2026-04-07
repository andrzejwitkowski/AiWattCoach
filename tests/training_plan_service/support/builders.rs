use super::{
    date_epoch, CallLog, FixedClock, InMemoryTrainingPlanOperationRepository,
    InMemoryTrainingPlanProjectedDayRepository, InMemoryTrainingPlanSnapshotRepository,
    StubTrainingPlanGenerator, StubWorkoutSummaryPort, TrainingPlanError,
    TrainingPlanGenerationOperation, TrainingPlanGenerationService, WorkoutRecap,
};

#[derive(Clone)]
pub(crate) struct BuiltService {
    pub(crate) service: TrainingPlanGenerationService<
        InMemoryTrainingPlanSnapshotRepository,
        InMemoryTrainingPlanProjectedDayRepository,
        InMemoryTrainingPlanOperationRepository,
        StubTrainingPlanGenerator,
        StubWorkoutSummaryPort,
        FixedClock,
    >,
    pub(crate) snapshots: InMemoryTrainingPlanSnapshotRepository,
    pub(crate) projected_days: InMemoryTrainingPlanProjectedDayRepository,
    pub(crate) operations: InMemoryTrainingPlanOperationRepository,
    pub(crate) generator: StubTrainingPlanGenerator,
    pub(crate) workout_summary: StubWorkoutSummaryPort,
}

pub(crate) fn build_service(
    call_log: CallLog,
    recap_responses: Vec<Result<WorkoutRecap, TrainingPlanError>>,
    initial_plan_responses: Vec<Result<String, TrainingPlanError>>,
    correction_responses: Vec<Result<String, TrainingPlanError>>,
    today: &str,
) -> BuiltService {
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log.clone(),
        recap_responses,
        initial_plan_responses,
        correction_responses,
    );
    let workout_summary = StubWorkoutSummaryPort::new(call_log);
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator.clone(),
        workout_summary.clone(),
        FixedClock {
            now_epoch_seconds: date_epoch(today),
        },
    );

    BuiltService {
        service,
        snapshots,
        projected_days,
        operations,
        generator,
        workout_summary,
    }
}

pub(crate) fn build_service_with_operation(
    call_log: CallLog,
    operation: TrainingPlanGenerationOperation,
    recap_responses: Vec<Result<WorkoutRecap, TrainingPlanError>>,
    initial_plan_responses: Vec<Result<String, TrainingPlanError>>,
    correction_responses: Vec<Result<String, TrainingPlanError>>,
    today: &str,
) -> BuiltService {
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations =
        InMemoryTrainingPlanOperationRepository::with_operation(call_log.clone(), operation);
    let generator = StubTrainingPlanGenerator::new(
        call_log.clone(),
        recap_responses,
        initial_plan_responses,
        correction_responses,
    );
    let workout_summary = StubWorkoutSummaryPort::new(call_log);
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator.clone(),
        workout_summary.clone(),
        FixedClock {
            now_epoch_seconds: date_epoch(today),
        },
    );

    BuiltService {
        service,
        snapshots,
        projected_days,
        operations,
        generator,
        workout_summary,
    }
}
