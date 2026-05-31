use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub(crate) use aiwattcoach::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
    identity::Clock,
    llm_tools::LlmToolLoopState,
    training_plan::{
        TrainingPlanConversationMessage, TrainingPlanConversationRole, TrainingPlanError,
        TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
        TrainingPlanGenerationOperationRepository, TrainingPlanGenerationService,
        TrainingPlanGenerator, TrainingPlanPhaseOutput, TrainingPlanPlanningContext,
        TrainingPlanProjectedDay, TrainingPlanProjectionRepository, TrainingPlanSnapshot,
        TrainingPlanSnapshotRepository, TrainingPlanToolLoopCheckpoint, TrainingPlanUseCases,
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
pub(crate) type PlanningContexts = Arc<Mutex<Vec<Option<TrainingPlanPlanningContext>>>>;
pub(crate) type RestoredToolLoopStates = Arc<Mutex<Vec<Option<LlmToolLoopState>>>>;

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
    planning_context: Arc<Mutex<Option<TrainingPlanPlanningContext>>>,
    call_log: CallLog,
}

impl StubWorkoutSummaryPort {
    pub(crate) fn new(call_log: CallLog) -> Self {
        Self {
            persisted_recaps: Arc::new(Mutex::new(Vec::new())),
            planning_context: Arc::new(Mutex::new(None)),
            call_log,
        }
    }

    pub(crate) fn persisted_recaps(&self) -> Vec<WorkoutRecap> {
        self.persisted_recaps.lock().unwrap().clone()
    }

    pub(crate) fn set_planning_context(
        &self,
        planning_context: Option<TrainingPlanPlanningContext>,
    ) {
        *self.planning_context.lock().unwrap() = planning_context;
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

    fn get_planning_context(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanPlanningContext>, TrainingPlanError>,
    > {
        push_call(&self.call_log, "workout_summary.get_planning_context");
        let planning_context = self.planning_context.lock().unwrap().clone();
        Box::pin(async move { Ok(planning_context) })
    }
}

#[derive(Clone)]
pub(crate) struct StubTrainingPlanGenerator {
    recap_responses: Arc<Mutex<VecDeque<Result<WorkoutRecap, TrainingPlanError>>>>,
    initial_plan_responses: Arc<Mutex<VecDeque<Result<String, TrainingPlanError>>>>,
    correction_responses: Arc<Mutex<VecDeque<Result<String, TrainingPlanError>>>>,
    initial_plan_descriptions: Arc<Mutex<VecDeque<Option<String>>>>,
    correction_descriptions: Arc<Mutex<VecDeque<Option<String>>>>,
    recap_calls: Arc<Mutex<u32>>,
    initial_plan_calls: Arc<Mutex<u32>>,
    correction_calls: Arc<Mutex<u32>>,
    correction_inputs: CorrectionInputs,
    initial_planning_contexts: PlanningContexts,
    correction_planning_contexts: PlanningContexts,
    initial_restored_states: RestoredToolLoopStates,
    correction_restored_states: RestoredToolLoopStates,
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
            initial_plan_descriptions: Arc::new(Mutex::new(VecDeque::new())),
            correction_descriptions: Arc::new(Mutex::new(VecDeque::new())),
            recap_calls: Arc::new(Mutex::new(0)),
            initial_plan_calls: Arc::new(Mutex::new(0)),
            correction_calls: Arc::new(Mutex::new(0)),
            correction_inputs: Arc::new(Mutex::new(Vec::new())),
            initial_planning_contexts: Arc::new(Mutex::new(Vec::new())),
            correction_planning_contexts: Arc::new(Mutex::new(Vec::new())),
            initial_restored_states: Arc::new(Mutex::new(Vec::new())),
            correction_restored_states: Arc::new(Mutex::new(Vec::new())),
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

    pub(crate) fn initial_planning_contexts(&self) -> Vec<Option<TrainingPlanPlanningContext>> {
        self.initial_planning_contexts.lock().unwrap().clone()
    }

    pub(crate) fn correction_planning_contexts(&self) -> Vec<Option<TrainingPlanPlanningContext>> {
        self.correction_planning_contexts.lock().unwrap().clone()
    }

    pub(crate) fn initial_restored_states(&self) -> Vec<Option<LlmToolLoopState>> {
        self.initial_restored_states.lock().unwrap().clone()
    }

    pub(crate) fn correction_restored_states(&self) -> Vec<Option<LlmToolLoopState>> {
        self.correction_restored_states.lock().unwrap().clone()
    }

    pub(crate) fn set_initial_plan_descriptions(&self, descriptions: Vec<Option<String>>) {
        *self.initial_plan_descriptions.lock().unwrap() = VecDeque::from(descriptions);
    }

    pub(crate) fn set_correction_descriptions(&self, descriptions: Vec<Option<String>>) {
        *self.correction_descriptions.lock().unwrap() = VecDeque::from(descriptions);
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

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        _checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        *self.initial_plan_calls.lock().unwrap() += 1;
        self.initial_planning_contexts
            .lock()
            .unwrap()
            .push(planning_context.cloned());
        self.initial_restored_states
            .lock()
            .unwrap()
            .push(restored_state);
        push_call(&self.call_log, "generator.generate_initial_plan_window");
        let response = self
            .initial_plan_responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected initial plan response");
        let description = self
            .initial_plan_descriptions
            .lock()
            .unwrap()
            .pop_front()
            .flatten();
        Box::pin(async move {
            response.map(|raw_response| TrainingPlanPhaseOutput {
                raw_response,
                description,
                tool_loop_state: LlmToolLoopState::default(),
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        raw_plan_response: &str,
        issues: Vec<ValidationIssue>,
        restored_state: Option<LlmToolLoopState>,
        _checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        *self.correction_calls.lock().unwrap() += 1;
        self.correction_planning_contexts
            .lock()
            .unwrap()
            .push(planning_context.cloned());
        self.correction_restored_states
            .lock()
            .unwrap()
            .push(restored_state);
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
        let description = self
            .correction_descriptions
            .lock()
            .unwrap()
            .pop_front()
            .flatten();
        Box::pin(async move {
            response.map(|raw_response| TrainingPlanPhaseOutput {
                raw_response,
                description,
                tool_loop_state: LlmToolLoopState::default(),
            })
        })
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

pub(crate) fn sample_planning_context() -> TrainingPlanPlanningContext {
    TrainingPlanPlanningContext {
        rpe: Some(6),
        messages: vec![
            TrainingPlanConversationMessage {
                role: TrainingPlanConversationRole::Coach,
                content: "Coach promised an easy recovery week with only light Z1 rides and no hard sessions unless truly necessary.".to_string(),
                created_at_epoch_seconds: 1_746_489_600,
            },
            TrainingPlanConversationMessage {
                role: TrainingPlanConversationRole::User,
                content: "That easy week structure sounds good.".to_string(),
                created_at_epoch_seconds: 1_746_490_200,
            },
        ],
    }
}
