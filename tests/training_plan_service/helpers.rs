use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub(crate) use aiwattcoach::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
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
pub(crate) use chrono::{NaiveDate, TimeZone, Utc};

pub(crate) const USER_ID: &str = "user-1";
pub(crate) const WORKOUT_ID: &str = "workout-1";
pub(crate) const MODEL: &str = "google/gemini-3-flash-preview";
pub(crate) const FIRST_DAY: &str = "2026-04-06";
pub(crate) const SECOND_DAY: &str = "2026-04-07";

pub(crate) type CallLog = Arc<Mutex<Vec<String>>>;
pub(crate) type CorrectionInputs = Arc<Mutex<Vec<(String, Vec<ValidationIssue>)>>>;

#[derive(Clone)]
pub(crate) struct FixedClock {
    pub(crate) now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}

#[derive(Clone)]
pub(crate) struct InMemoryTrainingPlanSnapshotRepository {
    pub(crate) snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanSnapshotRepository {
    pub(crate) fn new() -> Self {
        Self {
            snapshots: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn stored_snapshots(&self) -> Vec<TrainingPlanSnapshot> {
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
pub(crate) struct InMemoryTrainingPlanProjectedDayRepository {
    projected_days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
    snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanProjectedDayRepository {
    pub(crate) fn new(snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>) -> Self {
        Self {
            projected_days: Arc::new(Mutex::new(Vec::new())),
            snapshots,
        }
    }

    pub(crate) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.projected_days.lock().unwrap().clone()
    }

    pub(crate) fn store_snapshot_only(&self, snapshot: TrainingPlanSnapshot) {
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
            .filter(|day| day.user_id == user_id && day.superseded_at_epoch_seconds.is_none())
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
            .filter(|day| {
                day.operation_key == operation_key && day.superseded_at_epoch_seconds.is_none()
            })
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
                if day.superseded_at_epoch_seconds.is_some() {
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
                day.superseded_at_epoch_seconds = Some(replaced_at_epoch_seconds);
                day.updated_at_epoch_seconds = replaced_at_epoch_seconds;
            }

            for projected_day in &projected_days {
                if let Some(existing) = stored.iter_mut().find(|existing| {
                    existing.user_id == projected_day.user_id
                        && existing.operation_key == projected_day.operation_key
                        && existing.date == projected_day.date
                }) {
                    *existing = projected_day.clone();
                } else {
                    stored.push(projected_day.clone());
                }
            }
            let mut stored_snapshots = snapshots.lock().unwrap();
            if let Some(existing) = stored_snapshots
                .iter_mut()
                .find(|existing| existing.operation_key == snapshot.operation_key)
            {
                *existing = snapshot.clone();
            } else {
                stored_snapshots.push(snapshot.clone());
            }

            Ok((snapshot, projected_days))
        })
    }
}

#[derive(Clone)]
pub(crate) struct InMemoryTrainingPlanOperationRepository {
    operations: Arc<Mutex<Vec<TrainingPlanGenerationOperation>>>,
    call_log: CallLog,
}

impl InMemoryTrainingPlanOperationRepository {
    pub(crate) fn new(call_log: CallLog) -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    pub(crate) fn stored_operation(&self) -> TrainingPlanGenerationOperation {
        self.operations
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("expected stored operation")
    }

    pub(crate) fn with_operation(
        call_log: CallLog,
        operation: TrainingPlanGenerationOperation,
    ) -> Self {
        Self {
            operations: Arc::new(Mutex::new(vec![operation])),
            call_log,
        }
    }
}

#[derive(Clone)]
pub(crate) struct FailingUpsertTrainingPlanOperationRepository {
    operation: Arc<Mutex<Option<TrainingPlanGenerationOperation>>>,
    error_message: String,
}

impl FailingUpsertTrainingPlanOperationRepository {
    pub(crate) fn new(operation: TrainingPlanGenerationOperation, error_message: &str) -> Self {
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
pub(crate) struct StubWorkoutSummaryPort {
    persisted_recaps: Arc<Mutex<Vec<WorkoutRecap>>>,
    call_log: CallLog,
}

impl StubWorkoutSummaryPort {
    pub(crate) fn new(call_log: CallLog) -> Self {
        Self {
            persisted_recaps: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    pub(crate) fn persisted_recaps(&self) -> Vec<WorkoutRecap> {
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
pub(crate) struct StubTrainingPlanGenerator {
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
    pub(crate) fn new(
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

    pub(crate) fn recap_call_count(&self) -> u32 {
        *self.recap_calls.lock().unwrap()
    }

    pub(crate) fn initial_plan_call_count(&self) -> u32 {
        *self.initial_plan_calls.lock().unwrap()
    }

    pub(crate) fn correction_call_count(&self) -> u32 {
        *self.correction_calls.lock().unwrap()
    }

    pub(crate) fn correction_inputs(&self) -> Vec<(String, Vec<ValidationIssue>)> {
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

pub(crate) fn workout_recap() -> WorkoutRecap {
    WorkoutRecap::generated(
        "Steady aerobic ride with moderate fatigue.",
        "openrouter",
        MODEL,
        date_epoch(FIRST_DAY),
    )
}

pub(crate) fn valid_plan_window(start_date: &str) -> String {
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

pub(crate) fn plan_with_invalid_day(start_date: &str, invalid_date: &str) -> String {
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

pub(crate) fn window_with_duplicate_date(start_date: &str, duplicate_date: &str) -> String {
    let mut days = (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            format!("{date}\nEndurance\n- 45m 65%")
        })
        .collect::<Vec<_>>();
    days[5] = format!("{duplicate_date}\nTempo\n- 30m 80%");
    days.join("\n\n")
}

pub(crate) fn window_with_gap(start_date: &str, removed_date: &str) -> String {
    (0..14)
        .filter_map(|offset| {
            let date = add_days(start_date, offset);
            (date != removed_date).then(|| format!("{date}\nEndurance\n- 45m 65%"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn single_rest_day(date: &str) -> String {
    format!("{date}\nRest Day")
}

pub(crate) fn single_invalid_day(date: &str) -> String {
    format!("{date}\nBroken session\n- nope")
}

pub(crate) fn stale_pending_operation_with_checkpoints() -> TrainingPlanGenerationOperation {
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

pub(crate) fn stale_pending_operation_with_recap_only() -> TrainingPlanGenerationOperation {
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
        attempts: vec![AttemptRecord {
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

pub(crate) fn stale_pending_operation_with_invalid_correction_response(
) -> TrainingPlanGenerationOperation {
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

pub(crate) fn stale_pending_operation_with_snapshot_mismatch() -> TrainingPlanGenerationOperation {
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

pub(crate) fn snapshot_projected_days_for_first_day() -> Vec<TrainingPlanProjectedDay> {
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
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: date_epoch(FIRST_DAY),
            updated_at_epoch_seconds: date_epoch(FIRST_DAY),
        })
        .collect()
}

pub(crate) fn snapshot_for_first_day() -> TrainingPlanSnapshot {
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

pub(crate) fn valid_plan_window_days(
    start_date: &str,
) -> Vec<aiwattcoach::domain::training_plan::TrainingPlanDay> {
    let raw = valid_plan_window(start_date);
    let mut days = Vec::new();
    for block in raw.split("\n\n") {
        let parsed = aiwattcoach::domain::intervals::parse_planned_workout_days(block).unwrap();
        let day = parsed.days.into_iter().next().unwrap();
        let date = day.date.clone();
        let rest_day = day.is_rest_day();
        let workout = day.into_workout();
        days.push(aiwattcoach::domain::training_plan::TrainingPlanDay {
            date,
            rest_day,
            workout,
        });
    }
    days
}

pub(crate) fn date_epoch(date: &str) -> i64 {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    Utc.from_utc_datetime(&parsed.and_hms_opt(0, 0, 0).unwrap())
        .timestamp()
}

pub(crate) fn add_days(date: &str, offset: i64) -> String {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    parsed
        .checked_add_signed(chrono::Duration::days(offset))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}

pub(crate) fn new_call_log() -> CallLog {
    Arc::new(Mutex::new(Vec::new()))
}

pub(crate) fn push_call(call_log: &CallLog, call: &str) {
    call_log.lock().unwrap().push(call.to_string());
}

pub(crate) fn recorded_calls(call_log: &CallLog) -> Vec<String> {
    call_log.lock().unwrap().clone()
}

pub(crate) fn assert_event_order(calls: &[String], first: &str, second: &str) {
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
