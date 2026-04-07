use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus},
    identity::Clock,
    training_plan::{
        TrainingPlanError, TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
        TrainingPlanGenerationOperationRepository, TrainingPlanGenerationService,
        TrainingPlanGenerator, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanSnapshot, TrainingPlanSnapshotRepository, TrainingPlanUseCases,
        TrainingPlanWorkoutSummaryPort,
    },
    workout_summary::WorkoutRecap,
};
use chrono::{NaiveDate, TimeZone, Utc};

const USER_ID: &str = "user-1";
const WORKOUT_ID: &str = "workout-1";
const MODEL: &str = "google/gemini-3-flash-preview";
const FIRST_DAY: &str = "2026-04-06";
const SECOND_DAY: &str = "2026-04-07";

type CallLog = Arc<Mutex<Vec<String>>>;
type CorrectionInputs = Arc<Mutex<Vec<(String, Vec<ValidationIssue>)>>>;

#[derive(Clone)]
struct FixedClock {
    now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}

#[derive(Clone)]
struct InMemoryTrainingPlanSnapshotRepository {
    snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanSnapshotRepository {
    fn new() -> Self {
        Self {
            snapshots: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn stored_snapshots(&self) -> Vec<TrainingPlanSnapshot> {
        self.snapshots.lock().unwrap().clone()
    }
}

impl TrainingPlanSnapshotRepository for InMemoryTrainingPlanSnapshotRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanSnapshot>, TrainingPlanError>,
    > {
        let snapshot = self
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .find(|snapshot| snapshot.operation_key == operation_key)
            .cloned();
        Box::pin(async move { Ok(snapshot) })
    }
}

#[derive(Clone)]
struct InMemoryTrainingPlanProjectedDayRepository {
    projected_days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
    snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanProjectedDayRepository {
    fn new(snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>) -> Self {
        Self {
            projected_days: Arc::new(Mutex::new(Vec::new())),
            snapshots,
        }
    }

    fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.projected_days.lock().unwrap().clone()
    }

    fn store_snapshot_only(&self, snapshot: TrainingPlanSnapshot) {
        self.snapshots.lock().unwrap().push(snapshot);
    }
}

impl TrainingPlanProjectionRepository for InMemoryTrainingPlanProjectedDayRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let days = self
            .projected_days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.user_id == user_id && day.active)
            .cloned()
            .collect::<Vec<_>>();
        Box::pin(async move { Ok(days) })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let days = self
            .projected_days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.operation_key == operation_key && day.active)
            .cloned()
            .collect::<Vec<_>>();
        Box::pin(async move { Ok(days) })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
    > {
        let store = self.projected_days.clone();
        let snapshots = self.snapshots.clone();
        let today = today.to_string();
        Box::pin(async move {
            let mut stored = store.lock().unwrap();

            for day in stored.iter_mut() {
                if !day.active {
                    continue;
                }
                if day.user_id != snapshot.user_id {
                    continue;
                }
                if day.date < today
                    || day.date < snapshot.start_date
                    || day.date > snapshot.end_date
                {
                    continue;
                }
                day.active = false;
                day.superseded_at_epoch_seconds = Some(replaced_at_epoch_seconds);
                day.updated_at_epoch_seconds = replaced_at_epoch_seconds;
            }

            for projected_day in &projected_days {
                stored.push(projected_day.clone());
            }
            snapshots.lock().unwrap().push(snapshot.clone());

            Ok((
                snapshot,
                projected_days
                    .into_iter()
                    .filter(|day| day.active)
                    .collect(),
            ))
        })
    }
}

#[derive(Clone)]
struct InMemoryTrainingPlanOperationRepository {
    operations: Arc<Mutex<Vec<TrainingPlanGenerationOperation>>>,
    call_log: CallLog,
}

impl InMemoryTrainingPlanOperationRepository {
    fn new(call_log: CallLog) -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    fn stored_operation(&self) -> TrainingPlanGenerationOperation {
        self.operations
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("expected stored operation")
    }

    fn with_operation(call_log: CallLog, operation: TrainingPlanGenerationOperation) -> Self {
        Self {
            operations: Arc::new(Mutex::new(vec![operation])),
            call_log,
        }
    }
}

#[derive(Clone)]
struct FailingUpsertTrainingPlanOperationRepository {
    operation: Arc<Mutex<Option<TrainingPlanGenerationOperation>>>,
    error_message: String,
}

impl FailingUpsertTrainingPlanOperationRepository {
    fn new(operation: TrainingPlanGenerationOperation, error_message: &str) -> Self {
        Self {
            operation: Arc::new(Mutex::new(Some(operation))),
            error_message: error_message.to_string(),
        }
    }
}

impl TrainingPlanGenerationOperationRepository for InMemoryTrainingPlanOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>,
    > {
        let operation = self
            .operations
            .lock()
            .unwrap()
            .iter()
            .find(|operation| operation.operation_key == operation_key)
            .cloned();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationClaimResult, TrainingPlanError>,
    > {
        push_call(&self.call_log, "operation.claim_pending");
        let mut stored_operations = self.operations.lock().unwrap();
        let existing = stored_operations
            .iter()
            .find(|existing| existing.operation_key == operation.operation_key)
            .cloned();
        let result = match existing {
            None => {
                stored_operations.push(operation.clone());
                TrainingPlanGenerationClaimResult::Claimed(operation)
            }
            Some(existing)
                if existing.status == WorkflowStatus::Failed
                    || (existing.status == WorkflowStatus::Pending
                        && existing.last_attempt_at_epoch_seconds
                            <= stale_before_epoch_seconds) =>
            {
                let reclaimed = existing.reclaim(operation.last_attempt_at_epoch_seconds);
                if let Some(stored) = stored_operations
                    .iter_mut()
                    .find(|stored| stored.operation_key == reclaimed.operation_key)
                {
                    *stored = reclaimed.clone();
                }
                TrainingPlanGenerationClaimResult::Claimed(reclaimed)
            }
            Some(existing) => TrainingPlanGenerationClaimResult::Existing(existing),
        };
        Box::pin(async move { Ok(result) })
    }

    fn upsert(
        &self,
        operation: TrainingPlanGenerationOperation,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationOperation, TrainingPlanError>,
    > {
        push_call(&self.call_log, "operation.upsert");
        let store = self.operations.clone();
        Box::pin(async move {
            let mut operations = store.lock().unwrap();
            if let Some(existing) = operations
                .iter_mut()
                .find(|existing| existing.operation_key == operation.operation_key)
            {
                *existing = operation.clone();
            } else {
                operations.push(operation.clone());
            }
            Ok(operation)
        })
    }
}

impl TrainingPlanGenerationOperationRepository for FailingUpsertTrainingPlanOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>,
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
        Result<TrainingPlanGenerationClaimResult, TrainingPlanError>,
    > {
        Box::pin(async move { Ok(TrainingPlanGenerationClaimResult::Claimed(operation)) })
    }

    fn upsert(
        &self,
        _operation: TrainingPlanGenerationOperation,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationOperation, TrainingPlanError>,
    > {
        let error_message = self.error_message.clone();
        Box::pin(async move { Err(TrainingPlanError::Repository(error_message)) })
    }
}

#[derive(Clone)]
struct StubWorkoutSummaryPort {
    persisted_recaps: Arc<Mutex<Vec<WorkoutRecap>>>,
    call_log: CallLog,
}

impl StubWorkoutSummaryPort {
    fn new(call_log: CallLog) -> Self {
        Self {
            persisted_recaps: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    fn persisted_recaps(&self) -> Vec<WorkoutRecap> {
        self.persisted_recaps.lock().unwrap().clone()
    }
}

impl TrainingPlanWorkoutSummaryPort for StubWorkoutSummaryPort {
    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        recap: WorkoutRecap,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        push_call(&self.call_log, "workout_summary.persist_workout_recap");
        let store = self.persisted_recaps.clone();
        Box::pin(async move {
            store.lock().unwrap().push(recap);
            Ok(())
        })
    }
}

#[derive(Clone)]
struct StubTrainingPlanGenerator {
    recap_responses: Arc<Mutex<VecDeque<Result<WorkoutRecap, TrainingPlanError>>>>,
    initial_plan_responses: Arc<Mutex<VecDeque<Result<String, TrainingPlanError>>>>,
    correction_responses: Arc<Mutex<VecDeque<Result<String, TrainingPlanError>>>>,
    recap_calls: Arc<Mutex<u32>>,
    initial_plan_calls: Arc<Mutex<u32>>,
    correction_calls: Arc<Mutex<u32>>,
    correction_inputs: CorrectionInputs,
    call_log: CallLog,
}

impl StubTrainingPlanGenerator {
    fn new(
        call_log: CallLog,
        recap_responses: Vec<Result<WorkoutRecap, TrainingPlanError>>,
        initial_plan_responses: Vec<Result<String, TrainingPlanError>>,
        correction_responses: Vec<Result<String, TrainingPlanError>>,
    ) -> Self {
        Self {
            recap_responses: Arc::new(Mutex::new(VecDeque::from(recap_responses))),
            initial_plan_responses: Arc::new(Mutex::new(VecDeque::from(initial_plan_responses))),
            correction_responses: Arc::new(Mutex::new(VecDeque::from(correction_responses))),
            recap_calls: Arc::new(Mutex::new(0)),
            initial_plan_calls: Arc::new(Mutex::new(0)),
            correction_calls: Arc::new(Mutex::new(0)),
            correction_inputs: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    fn recap_call_count(&self) -> u32 {
        *self.recap_calls.lock().unwrap()
    }

    fn initial_plan_call_count(&self) -> u32 {
        *self.initial_plan_calls.lock().unwrap()
    }

    fn correction_call_count(&self) -> u32 {
        *self.correction_calls.lock().unwrap()
    }

    fn correction_inputs(&self) -> Vec<(String, Vec<ValidationIssue>)> {
        self.correction_inputs.lock().unwrap().clone()
    }
}

impl TrainingPlanGenerator for StubTrainingPlanGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        *self.recap_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "generator.generate_workout_recap");
        let response = self
            .recap_responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected recap response");
        Box::pin(async move { response })
    }

    fn generate_initial_plan_window(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<String, TrainingPlanError>> {
        *self.initial_plan_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "generator.generate_initial_plan_window");
        let response = self
            .initial_plan_responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected initial plan response");
        Box::pin(async move { response })
    }

    fn correct_invalid_days(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        raw_plan_response: &str,
        issues: Vec<ValidationIssue>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<String, TrainingPlanError>> {
        *self.correction_calls.lock().unwrap() += 1;
        self.correction_inputs
            .lock()
            .unwrap()
            .push((raw_plan_response.to_string(), issues));
        push_call(&self.call_log, "generator.correct_invalid_days");
        let response = self
            .correction_responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected correction response");
        Box::pin(async move { response })
    }
}

#[tokio::test]
async fn generates_snapshot_and_projected_days_for_saved_workout() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator.clone(),
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(result.snapshot.days.len(), 14);
    assert_eq!(result.active_projected_days.len(), 13);
    assert_eq!(snapshots.stored_snapshots().len(), 1);
    assert_eq!(
        projected_days
            .stored_days()
            .iter()
            .filter(|day| day.active)
            .count(),
        13
    );
    assert!(!projected_days
        .stored_days()
        .iter()
        .any(|day| day.date == FIRST_DAY && day.active));

    let operation = operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert_eq!(
        operation.operation_key,
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        )
    );
}

#[tokio::test]
async fn persists_workout_recap_before_generating_training_plan_window() {
    let call_log = new_call_log();
    let service = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    service
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "workout_summary.persist_workout_recap",
        "generator.generate_initial_plan_window",
    );
}

#[tokio::test]
async fn checkpoints_recap_in_operation_before_persisting_to_workout_summary() {
    let call_log = new_call_log();
    let built = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "operation.upsert",
        "workout_summary.persist_workout_recap",
    );
}

#[tokio::test]
async fn replay_of_same_saved_workout_generation_is_idempotent() {
    let call_log = new_call_log();
    let built = build_service(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    let first = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();
    let replay = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(first.was_generated);
    assert!(!replay.was_generated);
    assert_eq!(first.snapshot.operation_key, replay.snapshot.operation_key);
    assert_eq!(built.generator.recap_call_count(), 1);
    assert_eq!(built.generator.initial_plan_call_count(), 1);
    assert_eq!(built.snapshots.stored_snapshots().len(), 1);
    assert_eq!(built.projected_days.stored_days().len(), 14);
}

#[tokio::test]
async fn next_day_generation_supersedes_only_overlapping_future_projected_days() {
    let first = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );
    first
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let second_generator = StubTrainingPlanGenerator::new(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(SECOND_DAY))],
        vec![],
    );
    let second_service = TrainingPlanGenerationService::new(
        first.snapshots.clone(),
        first.projected_days.clone(),
        first.operations.clone(),
        second_generator,
        first.workout_summary.clone(),
        FixedClock {
            now_epoch_seconds: date_epoch(SECOND_DAY),
        },
    );

    second_service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(SECOND_DAY))
        .await
        .unwrap();

    let stored_days = first.projected_days.stored_days();
    let active_days = stored_days
        .iter()
        .filter(|day| day.active)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(active_days.len(), 13);
    assert!(!active_days.iter().any(|day| day.date == FIRST_DAY));
    assert!(!active_days.iter().any(|day| day.date == SECOND_DAY));
    assert!(stored_days.iter().any(|day| {
        day.date == SECOND_DAY
            && day.operation_key
                == format!(
                    "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
                    date_epoch(FIRST_DAY)
                )
            && !day.active
    }));
}

#[tokio::test]
async fn invalid_day_parse_records_date_scoped_validation_issue() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(
        operation.validation_issues[0],
        ValidationIssue {
            scope: "2026-04-10".to_string(),
            message: "failed to parse day 2026-04-10: invalid planned workout step: - nope"
                .to_string(),
        }
    );
}

#[tokio::test]
async fn correction_round_merges_corrected_days_and_succeeds() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(single_rest_day("2026-04-10"))],
        FIRST_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(built.generator.correction_call_count(), 1);
    let correction_inputs = built.generator.correction_inputs();
    assert_eq!(correction_inputs.len(), 1);
    assert_eq!(
        correction_inputs[0].0,
        "2026-04-10\nBroken session\n- nope".to_string()
    );
    assert_eq!(correction_inputs[0].1.len(), 1);
    let corrected_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-10")
        .expect("expected corrected day");
    assert!(corrected_day.rest_day);
    let untouched_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-09")
        .expect("expected untouched day");
    assert!(!untouched_day.rest_day);
    assert_eq!(result.snapshot.days.len(), 14);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert!(operation.validation_issues.is_empty());
}

#[tokio::test]
async fn correction_retry_exhaustion_marks_failed_operation_and_keeps_raw_responses() {
    let initial_raw = plan_with_invalid_day(FIRST_DAY, "2026-04-10");
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(initial_raw.clone())],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.raw_plan_response.as_deref(),
        Some(initial_raw.as_str())
    );
    assert!(operation.raw_correction_response.is_some());
    assert_eq!(built.generator.correction_call_count(), 2);
}

#[tokio::test]
async fn reclaim_with_stored_invalid_correction_response_keeps_full_retry_budget() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_invalid_correction_response(),
        vec![],
        vec![],
        vec![
            Ok(single_invalid_day("2026-04-10")),
            Ok(single_invalid_day("2026-04-10")),
        ],
        SECOND_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );
    assert_eq!(built.generator.correction_call_count(), 2);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(operation.validation_issues[0].scope, "2026-04-10");
}

#[tokio::test]
async fn correction_retry_persists_latest_invalid_scope_after_scope_shifts() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-11")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-11")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(operation.validation_issues[0].scope, "2026-04-11");

    let correction_inputs = built.generator.correction_inputs();
    assert_eq!(correction_inputs.len(), 2);
    assert_eq!(correction_inputs[0].1[0].scope, "2026-04-10");
    assert_eq!(correction_inputs[1].1[0].scope, "2026-04-11");
}

#[tokio::test]
async fn correction_ignores_changes_for_dates_that_were_not_invalid() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(
            "2026-04-10\nRest Day\n\n2026-04-09\nRest Day".to_string()
        )],
        FIRST_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let corrected_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-10")
        .expect("expected corrected day");
    assert!(corrected_day.rest_day);

    let untouched_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-09")
        .expect("expected untouched day");
    assert!(!untouched_day.rest_day);
}

#[tokio::test]
async fn duplicate_dates_fail_durably_instead_of_overwriting() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(window_with_duplicate_date(FIRST_DAY, "2026-04-10"))],
        vec![],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Validation("duplicate planned workout day: 2026-04-10".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.failure.unwrap().phase,
        WorkflowPhase::InitialGeneration
    );
}

#[tokio::test]
async fn non_contiguous_windows_fail_durably() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(window_with_gap(FIRST_DAY, "2026-04-12"))],
        vec![],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Validation(
            "training plan window must contain exactly 14 contiguous dated days".to_string()
        )
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.failure.unwrap().phase,
        WorkflowPhase::InitialGeneration
    );
}

#[tokio::test]
async fn reclaim_resumes_from_stored_checkpoints_without_regenerating_completed_phases() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_checkpoints(),
        vec![],
        vec![],
        vec![],
        SECOND_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(built.generator.recap_call_count(), 0);
    assert_eq!(built.generator.initial_plan_call_count(), 0);
    assert_eq!(built.generator.correction_call_count(), 0);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert!(operation.validation_issues.is_empty());
}

#[tokio::test]
async fn reclaim_with_stored_recap_repersists_workout_summary_before_initial_plan_generation() {
    let call_log = new_call_log();
    let built = build_service_with_operation(
        call_log.clone(),
        stale_pending_operation_with_recap_only(),
        vec![],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        SECOND_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_eq!(built.generator.recap_call_count(), 0);
    assert_eq!(built.workout_summary.persisted_recaps().len(), 1);
    assert_eq!(
        built.workout_summary.persisted_recaps()[0].generated_at_epoch_seconds,
        date_epoch(FIRST_DAY) - 600
    );
    assert_event_order(
        &recorded_calls(&call_log),
        "workout_summary.persist_workout_recap",
        "generator.generate_initial_plan_window",
    );

    let operation = built.operations.stored_operation();
    let recap_attempts = operation
        .attempts
        .iter()
        .filter(|attempt| attempt.phase == WorkflowPhase::WorkoutRecap)
        .count();
    assert_eq!(recap_attempts, 1);
}

#[tokio::test]
async fn failed_operation_persistence_error_is_surfaced() {
    let operation = TrainingPlanGenerationOperation::pending(
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        USER_ID.to_string(),
        WORKOUT_ID.to_string(),
        date_epoch(FIRST_DAY),
        date_epoch(FIRST_DAY),
    );
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = FailingUpsertTrainingPlanOperationRepository::new(
        operation,
        "operation failure write failed",
    );
    let generator = StubTrainingPlanGenerator::new(
        new_call_log(),
        vec![Err(TrainingPlanError::Validation(
            "recap generation failed".to_string(),
        ))],
        vec![],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations,
        generator,
        StubWorkoutSummaryPort::new(new_call_log()),
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );

    let error = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Repository("operation failure write failed".to_string())
    );
}

#[tokio::test]
async fn fail_operation_preserves_unavailable_error_kind() {
    let built = build_service(
        new_call_log(),
        vec![Err(TrainingPlanError::Unavailable(
            "provider timed out".to_string(),
        ))],
        vec![],
        vec![],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("provider timed out".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
}

#[tokio::test]
async fn heals_pending_operation_when_snapshot_already_exists() {
    let call_log = new_call_log();
    let built = build_service_with_operation(
        call_log,
        stale_pending_operation_with_snapshot_mismatch(),
        vec![],
        vec![],
        vec![],
        FIRST_DAY,
    );

    let snapshot = snapshot_for_first_day();
    built
        .projected_days
        .replace_window(
            snapshot,
            snapshot_projected_days_for_first_day(),
            FIRST_DAY,
            date_epoch(FIRST_DAY),
        )
        .await
        .unwrap();

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(!result.was_generated);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
}

#[tokio::test]
async fn replay_does_not_heal_pending_operation_when_snapshot_exists_without_projected_days() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_snapshot_mismatch(),
        vec![],
        vec![],
        vec![],
        FIRST_DAY,
    );

    built
        .projected_days
        .store_snapshot_only(snapshot_for_first_day());

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(result.active_projected_days.len(), 13);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert_eq!(built.projected_days.stored_days().len(), 14);
}

#[tokio::test]
async fn successful_generation_records_real_workflow_attempts() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(single_rest_day("2026-04-10"))],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let operation = built.operations.stored_operation();
    let phases = operation
        .attempts
        .iter()
        .map(|attempt| attempt.phase.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            WorkflowPhase::WorkoutRecap,
            WorkflowPhase::InitialGeneration,
            WorkflowPhase::Correction,
            WorkflowPhase::ProjectionUpdate,
        ]
    );
}

#[derive(Clone)]
struct BuiltService {
    service: TrainingPlanGenerationService<
        InMemoryTrainingPlanSnapshotRepository,
        InMemoryTrainingPlanProjectedDayRepository,
        InMemoryTrainingPlanOperationRepository,
        StubTrainingPlanGenerator,
        StubWorkoutSummaryPort,
        FixedClock,
    >,
    snapshots: InMemoryTrainingPlanSnapshotRepository,
    projected_days: InMemoryTrainingPlanProjectedDayRepository,
    operations: InMemoryTrainingPlanOperationRepository,
    generator: StubTrainingPlanGenerator,
    workout_summary: StubWorkoutSummaryPort,
}

fn build_service(
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

fn build_service_with_operation(
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

fn workout_recap() -> WorkoutRecap {
    WorkoutRecap::generated(
        "Steady aerobic ride with moderate fatigue.",
        "openrouter",
        MODEL,
        date_epoch(FIRST_DAY),
    )
}

fn valid_plan_window(start_date: &str) -> String {
    (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            if offset % 4 == 0 {
                format!("{date}\nRest Day")
            } else {
                format!("{date}\nEndurance\n- 45m 65%")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn plan_with_invalid_day(start_date: &str, invalid_date: &str) -> String {
    (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            if date == invalid_date {
                format!("{date}\nBroken session\n- nope")
            } else if offset % 4 == 0 {
                format!("{date}\nRest Day")
            } else {
                format!("{date}\nEndurance\n- 45m 65%")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn window_with_duplicate_date(start_date: &str, duplicate_date: &str) -> String {
    let mut days = (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            format!("{date}\nEndurance\n- 45m 65%")
        })
        .collect::<Vec<_>>();
    days[5] = format!("{duplicate_date}\nTempo\n- 30m 80%");
    days.join("\n\n")
}

fn window_with_gap(start_date: &str, removed_date: &str) -> String {
    (0..14)
        .filter_map(|offset| {
            let date = add_days(start_date, offset);
            (date != removed_date).then(|| format!("{date}\nEndurance\n- 45m 65%"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn single_rest_day(date: &str) -> String {
    format!("{date}\nRest Day")
}

fn single_invalid_day(date: &str) -> String {
    format!("{date}\nBroken session\n- nope")
}

fn stale_pending_operation_with_checkpoints() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        raw_correction_response: Some(single_rest_day("2026-04-10")),
        validation_issues: Vec::new(),
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

fn stale_pending_operation_with_recap_only() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: None,
        raw_correction_response: None,
        validation_issues: Vec::new(),
        attempts: vec![aiwattcoach::domain::ai_workflow::AttemptRecord {
            phase: WorkflowPhase::WorkoutRecap,
            attempt_number: 1,
            recorded_at_epoch_seconds: date_epoch(FIRST_DAY),
        }],
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(SECOND_DAY),
    }
}

fn stale_pending_operation_with_invalid_correction_response() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        raw_correction_response: Some(single_invalid_day("2026-04-10")),
        validation_issues: vec![ValidationIssue {
            scope: "2026-04-10".to_string(),
            message: "invalid planned workout step: - nope".to_string(),
        }],
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(SECOND_DAY),
    }
}

fn stale_pending_operation_with_snapshot_mismatch() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(valid_plan_window(FIRST_DAY)),
        raw_correction_response: None,
        validation_issues: Vec::new(),
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

fn snapshot_projected_days_for_first_day() -> Vec<TrainingPlanProjectedDay> {
    let snapshot = snapshot_for_first_day();

    snapshot
        .days
        .iter()
        .map(|day| TrainingPlanProjectedDay {
            user_id: snapshot.user_id.clone(),
            workout_id: snapshot.workout_id.clone(),
            operation_key: snapshot.operation_key.clone(),
            date: day.date.clone(),
            rest_day: day.rest_day,
            workout: day.workout.clone(),
            active: day.date.as_str() > FIRST_DAY,
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: date_epoch(FIRST_DAY),
            updated_at_epoch_seconds: date_epoch(FIRST_DAY),
        })
        .collect()
}

fn snapshot_for_first_day() -> TrainingPlanSnapshot {
    TrainingPlanSnapshot {
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        start_date: FIRST_DAY.to_string(),
        end_date: add_days(FIRST_DAY, 13),
        days: valid_plan_window_days(FIRST_DAY),
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

fn valid_plan_window_days(
    start_date: &str,
) -> Vec<aiwattcoach::domain::training_plan::TrainingPlanDay> {
    let raw = valid_plan_window(start_date);
    let mut days = Vec::new();
    for block in raw.split("\n\n") {
        let parsed = aiwattcoach::domain::intervals::parse_planned_workout_days(block).unwrap();
        let day = parsed.days.into_iter().next().unwrap();
        days.push(aiwattcoach::domain::training_plan::TrainingPlanDay {
            date: day.date,
            rest_day: day.rest_day,
            workout: day.workout,
        });
    }
    days
}

fn date_epoch(date: &str) -> i64 {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    Utc.from_utc_datetime(&parsed.and_hms_opt(0, 0, 0).unwrap())
        .timestamp()
}

fn add_days(date: &str, offset: i64) -> String {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    parsed
        .checked_add_signed(chrono::Duration::days(offset))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}

fn new_call_log() -> CallLog {
    Arc::new(Mutex::new(Vec::new()))
}

fn push_call(call_log: &CallLog, call: &str) {
    call_log.lock().unwrap().push(call.to_string());
}

fn recorded_calls(call_log: &CallLog) -> Vec<String> {
    call_log.lock().unwrap().clone()
}

fn assert_event_order(calls: &[String], first: &str, second: &str) {
    let first_index = calls
        .iter()
        .position(|call| call == first)
        .unwrap_or_else(|| panic!("missing call: {first}"));
    let second_index = calls
        .iter()
        .position(|call| call == second)
        .unwrap_or_else(|| panic!("missing call: {second}"));

    assert!(
        first_index < second_index,
        "expected {first} before {second}, got {calls:?}"
    );
}
