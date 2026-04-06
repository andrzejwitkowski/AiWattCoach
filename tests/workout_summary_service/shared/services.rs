use super::*;

pub(crate) fn test_service(
    repository: InMemoryWorkoutSummaryRepository,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::new(
        repository,
        InMemoryCoachReplyOperationRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
}

pub(crate) fn test_service_with_coach(
    repository: InMemoryWorkoutSummaryRepository,
    reply_operations: InMemoryCoachReplyOperationRepository,
    coach: Arc<dyn WorkoutCoach>,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::with_coach(
        repository,
        reply_operations,
        TestClock,
        TestIdGenerator::default(),
        coach,
    )
}

pub(crate) fn test_service_with_coach_and_athlete_summary(
    repository: InMemoryWorkoutSummaryRepository,
    reply_operations: InMemoryCoachReplyOperationRepository,
    coach: Arc<dyn WorkoutCoach>,
    athlete_summary_service: Arc<dyn AthleteSummaryUseCases>,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::with_coach(
        repository,
        reply_operations,
        TestClock,
        TestIdGenerator::default(),
        coach,
    )
    .with_athlete_summary_service(athlete_summary_service)
}

pub(crate) fn test_service_with_training_plan(
    repository: InMemoryWorkoutSummaryRepository,
    training_plan_service: Arc<dyn TrainingPlanUseCases>,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::new(
        repository,
        InMemoryCoachReplyOperationRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_training_plan_service(training_plan_service)
}

pub(crate) fn default_dev_coach() -> Arc<dyn WorkoutCoach> {
    Arc::new(DevWorkoutCoach)
}

pub(crate) fn existing_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

#[derive(Clone, Default)]
pub(crate) struct RecordingTrainingPlanService {
    calls: Arc<Mutex<Vec<String>>>,
    next_result: Arc<Mutex<Option<Result<GeneratedTrainingPlan, TrainingPlanError>>>>,
}

impl RecordingTrainingPlanService {
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn fail_next(&self, error: TrainingPlanError) {
        *self.next_result.lock().unwrap() = Some(Err(error));
    }
}

impl TrainingPlanUseCases for RecordingTrainingPlanService {
    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<GeneratedTrainingPlan, TrainingPlanError>,
    > {
        let calls = self.calls.clone();
        let next_result = self.next_result.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "generate_for_saved_workout:{user_id}:{workout_id}:{saved_at_epoch_seconds}"
            ));
            if let Some(result) = next_result.lock().unwrap().take() {
                return result;
            }
            Err(TrainingPlanError::Unavailable(
                "training plan result not seeded in test".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
struct DevWorkoutCoach;

impl WorkoutCoach for DevWorkoutCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
        _athlete_summary_text: Option<&str>,
    ) -> aiwattcoach::domain::llm::BoxFuture<
        Result<aiwattcoach::domain::llm::LlmChatResponse, aiwattcoach::domain::llm::LlmError>,
    > {
        let message = format!(
            "Thanks, that helps. What stood out most about \"{user_message}\" during the workout?"
        );
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "dev-llm-coach".to_string(),
                message,
                provider_request_id: Some("dev-request-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            })
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct StubAthleteSummaryService {
    summary: Option<AthleteSummary>,
    stale: bool,
    regenerated_summary_text: Option<String>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl StubAthleteSummaryService {
    pub(crate) fn fresh(summary_text: &str) -> Self {
        Self {
            summary: Some(AthleteSummary {
                user_id: "user-1".to_string(),
                summary_text: summary_text.to_string(),
                generated_at_epoch_seconds: 1_700_000_000,
                created_at_epoch_seconds: 1_700_000_000,
                updated_at_epoch_seconds: 1_700_000_000,
                provider: Some("openrouter".to_string()),
                model: Some("google/gemini-3-flash-preview".to_string()),
            }),
            stale: false,
            regenerated_summary_text: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn stale(summary_text: &str) -> Self {
        let mut service = Self::fresh(summary_text);
        service.stale = true;
        service.regenerated_summary_text = Some(format!("{summary_text} (regenerated)"));
        service
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl AthleteSummaryUseCases for StubAthleteSummaryService {
    fn get_summary_state(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
        let summary = self.summary.clone();
        let stale = self.stale;
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push("get_summary_state".to_string());
            Ok(AthleteSummaryState { summary, stale })
        })
    }

    fn generate_summary(
        &self,
        _user_id: &str,
        _force: bool,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        unreachable!()
    }

    fn ensure_fresh_summary(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        unreachable!()
    }

    fn ensure_fresh_summary_state(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>> {
        let mut summary = self.summary.clone().expect("summary should exist in test");
        let stale = self.stale;
        let regenerated_summary_text = self.regenerated_summary_text.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push("ensure_fresh_summary_state".to_string());
            if stale {
                if let Some(text) = regenerated_summary_text {
                    summary.summary_text = text;
                }
            }
            Ok(EnsuredAthleteSummary {
                summary,
                was_regenerated: stale,
            })
        })
    }
}
