use std::sync::atomic::{AtomicBool, Ordering};

use aiwattcoach::domain::settings::Weekday;

use super::*;

#[derive(Clone)]
pub(crate) struct TestAvailabilitySettingsService {
    configured: bool,
}

impl TestAvailabilitySettingsService {
    pub(crate) fn configured() -> Arc<dyn aiwattcoach::domain::settings::UserSettingsUseCases> {
        Arc::new(Self { configured: true })
    }

    pub(crate) fn unconfigured() -> Arc<dyn aiwattcoach::domain::settings::UserSettingsUseCases> {
        Arc::new(Self { configured: false })
    }
}

impl aiwattcoach::domain::settings::UserSettingsUseCases for TestAvailabilitySettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            Option<aiwattcoach::domain::settings::UserSettings>,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings =
                aiwattcoach::domain::settings::UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = aiwattcoach::domain::settings::AvailabilitySettings {
                    configured: true,
                    days: vec![
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Mon,
                            available: true,
                            max_duration_minutes: Some(60),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Tue,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Wed,
                            available: true,
                            max_duration_minutes: Some(90),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Thu,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Fri,
                            available: true,
                            max_duration_minutes: Some(120),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Sat,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Sun,
                            available: false,
                            max_duration_minutes: None,
                        },
                    ],
                };
            }
            Ok(Some(settings))
        })
    }

    fn get_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings =
                aiwattcoach::domain::settings::UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = aiwattcoach::domain::settings::AvailabilitySettings {
                    configured: true,
                    days: vec![
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Mon,
                            available: true,
                            max_duration_minutes: Some(60),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Tue,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Wed,
                            available: true,
                            max_duration_minutes: Some(90),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Thu,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Fri,
                            available: true,
                            max_duration_minutes: Some(120),
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Sat,
                            available: false,
                            max_duration_minutes: None,
                        },
                        aiwattcoach::domain::settings::AvailabilityDay {
                            weekday: Weekday::Sun,
                            available: false,
                            max_duration_minutes: None,
                        },
                    ],
                };
            }
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: aiwattcoach::domain::settings::AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: aiwattcoach::domain::settings::IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: aiwattcoach::domain::settings::AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: aiwattcoach::domain::settings::AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: aiwattcoach::domain::settings::CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }
}

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

pub(crate) fn test_service_with_settings(
    repository: InMemoryWorkoutSummaryRepository,
    settings_service: Arc<dyn aiwattcoach::domain::settings::UserSettingsUseCases>,
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
    .with_settings_service(settings_service)
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

pub(crate) fn test_service_with_training_plan_and_latest_activity(
    repository: InMemoryWorkoutSummaryRepository,
    training_plan_service: Arc<dyn TrainingPlanUseCases>,
    latest_completed_activity_service: Arc<
        dyn aiwattcoach::domain::workout_summary::LatestCompletedActivityUseCases,
    >,
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
    .with_latest_completed_activity_service(latest_completed_activity_service)
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

pub(crate) fn existing_summary_with_finished_conversation() -> WorkoutSummary {
    let mut summary = existing_summary();
    summary.messages.push(ConversationMessage {
        id: "message-coach-1".to_string(),
        role: MessageRole::Coach,
        content: "Nice work. Save this and we can build the next block.".to_string(),
        created_at_epoch_seconds: 1_700_000_050,
    });
    summary
}

#[derive(Clone, Default)]
pub(crate) struct RecordingTrainingPlanService {
    calls: Arc<Mutex<Vec<String>>>,
    next_result: Arc<Mutex<Option<Result<GeneratedTrainingPlan, TrainingPlanError>>>>,
    observed_persisted_saved_at: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub(crate) struct RecordingLatestCompletedActivityService {
    latest_activity_id: Arc<Mutex<Option<String>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingLatestCompletedActivityService {
    pub(crate) fn new(latest_activity_id: Option<&str>) -> Self {
        Self {
            latest_activity_id: Arc::new(Mutex::new(latest_activity_id.map(str::to_string))),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl aiwattcoach::domain::workout_summary::LatestCompletedActivityUseCases
    for RecordingLatestCompletedActivityService
{
    fn latest_completed_activity_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::workout_summary::BoxFuture<Result<Option<String>, WorkoutSummaryError>>
    {
        let latest_activity_id = self.latest_activity_id.lock().unwrap().clone();
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("latest_completed_activity_id:{user_id}"));
            Ok(latest_activity_id)
        })
    }
}

impl RecordingTrainingPlanService {
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn fail_next(&self, error: TrainingPlanError) {
        *self.next_result.lock().unwrap() = Some(Err(error));
    }

    pub(crate) fn succeed_next(&self, result: GeneratedTrainingPlan) {
        *self.next_result.lock().unwrap() = Some(Ok(result));
    }

    pub(crate) fn observed_persisted_saved_at(&self) -> bool {
        self.observed_persisted_saved_at.load(Ordering::Relaxed)
    }
}

impl TrainingPlanUseCases for RecordingTrainingPlanService {
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "generate_recap_for_saved_workout:{user_id}:{workout_id}:{saved_at_epoch_seconds}"
            ));
            Ok(WorkoutRecap::generated(
                "Saved workout recap".to_string(),
                "openrouter".to_string(),
                "google/gemini-3-flash-preview".to_string(),
                saved_at_epoch_seconds,
            ))
        })
    }

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

#[derive(Clone)]
pub(crate) struct PersistCheckingTrainingPlanService {
    repository: InMemoryWorkoutSummaryRepository,
    delegate: RecordingTrainingPlanService,
}

impl PersistCheckingTrainingPlanService {
    pub(crate) fn new(repository: InMemoryWorkoutSummaryRepository) -> Self {
        Self {
            repository,
            delegate: RecordingTrainingPlanService::default(),
        }
    }

    pub(crate) fn observed_persisted_saved_at(&self) -> bool {
        self.delegate.observed_persisted_saved_at()
    }
}

#[derive(Clone)]
pub(crate) struct RefreshingTrainingPlanService {
    repository: InMemoryWorkoutSummaryRepository,
    delegate: RecordingTrainingPlanService,
    refreshed_summary: WorkoutSummary,
    failure_after_refresh: Option<TrainingPlanError>,
}

impl RefreshingTrainingPlanService {
    pub(crate) fn new(
        repository: InMemoryWorkoutSummaryRepository,
        refreshed_summary: WorkoutSummary,
    ) -> Self {
        Self {
            repository,
            delegate: RecordingTrainingPlanService::default(),
            refreshed_summary,
            failure_after_refresh: None,
        }
    }

    pub(crate) fn new_with_failure(
        repository: InMemoryWorkoutSummaryRepository,
        refreshed_summary: WorkoutSummary,
        error: TrainingPlanError,
    ) -> Self {
        Self {
            repository,
            delegate: RecordingTrainingPlanService::default(),
            refreshed_summary,
            failure_after_refresh: Some(error),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.delegate.calls()
    }

    pub(crate) fn succeed_next(&self, result: GeneratedTrainingPlan) {
        self.delegate.succeed_next(result);
    }
}

impl TrainingPlanUseCases for RefreshingTrainingPlanService {
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        let delegate = self.delegate.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            delegate
                .generate_recap_for_saved_workout(&user_id, &workout_id, saved_at_epoch_seconds)
                .await
        })
    }

    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<GeneratedTrainingPlan, TrainingPlanError>,
    > {
        let repository = self.repository.clone();
        let delegate = self.delegate.clone();
        let refreshed_summary = self.refreshed_summary.clone();
        let failure_after_refresh = self.failure_after_refresh.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let result = delegate
                .generate_for_saved_workout(&user_id, &workout_id, saved_at_epoch_seconds)
                .await;
            repository.overwrite_summary(refreshed_summary);
            match result {
                Ok(result) => {
                    if let Some(error) = failure_after_refresh {
                        Err(error)
                    } else {
                        Ok(result)
                    }
                }
                Err(error) => Err(error),
            }
        })
    }
}

impl TrainingPlanUseCases for PersistCheckingTrainingPlanService {
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        let repository = self.repository.clone();
        let delegate = self.delegate.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let persisted_saved_at = repository
                .find_by_user_id_and_workout_id(&user_id, &workout_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .and_then(|summary| summary.saved_at_epoch_seconds);
            delegate.observed_persisted_saved_at.store(
                persisted_saved_at == Some(saved_at_epoch_seconds),
                Ordering::Relaxed,
            );
            delegate
                .generate_recap_for_saved_workout(&user_id, &workout_id, saved_at_epoch_seconds)
                .await
        })
    }

    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<GeneratedTrainingPlan, TrainingPlanError>,
    > {
        let repository = self.repository.clone();
        let delegate = self.delegate.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let persisted_saved_at = repository
                .find_by_user_id_and_workout_id(&user_id, &workout_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .and_then(|summary| summary.saved_at_epoch_seconds);
            delegate.observed_persisted_saved_at.store(
                persisted_saved_at == Some(saved_at_epoch_seconds),
                Ordering::Relaxed,
            );
            delegate.calls.lock().unwrap().push(format!(
                "generate_for_saved_workout:{user_id}:{workout_id}:{saved_at_epoch_seconds}"
            ));
            delegate.next_result.lock().unwrap().take().unwrap_or(Err(
                TrainingPlanError::Unavailable(
                    "training plan result not seeded in test".to_string(),
                ),
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
