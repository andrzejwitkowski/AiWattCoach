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
pub(crate) use chrono::{NaiveDate, Utc};

pub(crate) mod assertions;
pub(crate) mod builders;
pub(crate) mod constants;
pub(crate) mod fixtures;
pub(crate) mod repos;

pub(crate) use assertions::*;
pub(crate) use builders::*;
pub(crate) use constants::*;
pub(crate) use fixtures::*;
pub(crate) use repos::*;

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

pub(crate) fn new_call_log() -> CallLog {
    Arc::new(Mutex::new(Vec::new()))
}

pub(crate) fn push_call(call_log: &CallLog, call: &str) {
    call_log.lock().unwrap().push(call.to_string());
}

pub(crate) fn recorded_calls(call_log: &CallLog) -> Vec<String> {
    call_log.lock().unwrap().clone()
}
