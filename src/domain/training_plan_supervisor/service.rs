use crate::domain::{
    identity::Clock,
    settings::{UserSettingsUseCases, DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL},
    training_plan::{TrainingPlanError, TrainingPlanProjectionRepository},
};

use super::{
    BoxFuture, GeminiSupervisorWebhookOutcome, TrainingPlanSupervisorBatchPort,
    TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
    TrainingPlanSupervisorReview, TrainingPlanSupervisorScheduler,
};

struct GeminiBatchWebhookInput {
    worker_operation_key: String,
    provided_webhook_token: String,
    expected_webhook_token: Option<String>,
    event_type: String,
    batch_name: String,
}

#[derive(Clone, Default)]
pub struct NoopTrainingPlanSupervisorScheduler;

impl TrainingPlanSupervisorScheduler for NoopTrainingPlanSupervisorScheduler {
    fn initialize_pending_review(
        &self,
        _user_id: &str,
        _worker_operation_key: &str,
        _worker_saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        Box::pin(async { Ok(None) })
    }
}

#[derive(Clone)]
pub struct TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    repository: Repo,
    settings: Settings,
    clock: Time,
}

impl<Repo, Settings, Time> TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    pub fn new(repository: Repo, settings: Settings, clock: Time) -> Self {
        Self {
            repository,
            settings,
            clock,
        }
    }

    pub fn complete_review<Projections>(
        &self,
        projections: Projections,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
    {
        let repository = self.repository.clone();
        let worker_operation_key = worker_operation_key.to_string();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let existing = repository
                .find_by_worker_operation_key(&worker_operation_key)
                .await?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;
            let completed = existing.complete_review(review, now_epoch_seconds)?;
            let completed = repository.upsert(completed).await?;
            projections
                .update_supervisor_status(
                    &completed.user_id,
                    &completed.worker_operation_key,
                    Some(completed.status),
                    completed.updated_at_epoch_seconds,
                )
                .await?;
            Ok(completed)
        })
    }

    fn handle_gemini_batch_webhook<Projections, Batch>(
        &self,
        projections: Projections,
        batch: Batch,
        input: GeminiBatchWebhookInput,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
        Batch: TrainingPlanSupervisorBatchPort + Clone,
    {
        let settings = self.settings.clone();
        let service = self.clone();
        Box::pin(async move {
            let Some(expected_webhook_token) = input.expected_webhook_token else {
                return Err(TrainingPlanError::Unavailable(
                    "Gemini supervisor webhook is not configured".to_string(),
                ));
            };
            if expected_webhook_token != input.provided_webhook_token {
                return Err(TrainingPlanError::Validation(
                    "Gemini supervisor webhook token is invalid".to_string(),
                ));
            }
            if input.event_type != "batch.succeeded" {
                return Ok(GeminiSupervisorWebhookOutcome::Ignored);
            }

            let operation = service
                .repository
                .find_by_worker_operation_key(&input.worker_operation_key)
                .await?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {} not found",
                        input.worker_operation_key
                    ))
                })?;
            let user_settings = settings
                .get_settings(&operation.user_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let gemini_api_key = user_settings
                .ai_agents
                .gemini_api_key
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    TrainingPlanError::Unavailable(
                        "Gemini supervisor webhook requires a configured Gemini API key"
                            .to_string(),
                    )
                })?;
            let review = batch
                .download_result(&gemini_api_key, &input.batch_name)
                .await?;
            let completed = service
                .complete_review(projections, &input.worker_operation_key, review)
                .await?;
            Ok(GeminiSupervisorWebhookOutcome::Accepted(completed))
        })
    }
}

pub trait TrainingPlanSupervisorWebhookUseCases: Send + Sync {
    fn receive_gemini_batch_webhook(
        &self,
        worker_operation_key: &str,
        provided_webhook_token: &str,
        event_type: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>>;
}

#[derive(Clone)]
pub struct GeminiTrainingPlanSupervisorWebhookService<Repo, Settings, Time, Projections, Batch>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
{
    supervisor: TrainingPlanSupervisorService<Repo, Settings, Time>,
    projections: Projections,
    batch: Batch,
    webhook_token: Option<String>,
}

impl<Repo, Settings, Time, Projections, Batch>
    GeminiTrainingPlanSupervisorWebhookService<Repo, Settings, Time, Projections, Batch>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
{
    pub fn new(
        supervisor: TrainingPlanSupervisorService<Repo, Settings, Time>,
        projections: Projections,
        batch: Batch,
        webhook_token: Option<String>,
    ) -> Self {
        Self {
            supervisor,
            projections,
            batch,
            webhook_token,
        }
    }
}

impl<Repo, Settings, Time, Projections, Batch> TrainingPlanSupervisorWebhookUseCases
    for GeminiTrainingPlanSupervisorWebhookService<Repo, Settings, Time, Projections, Batch>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
{
    fn receive_gemini_batch_webhook(
        &self,
        worker_operation_key: &str,
        provided_webhook_token: &str,
        event_type: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>> {
        self.supervisor.handle_gemini_batch_webhook(
            self.projections.clone(),
            self.batch.clone(),
            GeminiBatchWebhookInput {
                worker_operation_key: worker_operation_key.to_string(),
                provided_webhook_token: provided_webhook_token.to_string(),
                expected_webhook_token: self.webhook_token.clone(),
                event_type: event_type.to_string(),
                batch_name: batch_name.to_string(),
            },
        )
    }
}

impl<Repo, Settings, Time> TrainingPlanSupervisorScheduler
    for TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    fn initialize_pending_review(
        &self,
        user_id: &str,
        worker_operation_key: &str,
        worker_saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        let repository = self.repository.clone();
        let settings = self.settings.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            if let Some(existing) = repository
                .find_by_worker_operation_key(&worker_operation_key)
                .await?
            {
                return Ok(Some(existing));
            }

            let settings = settings
                .find_settings(&user_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let Some(settings) = settings else {
                return Ok(None);
            };
            if !settings.ai_agents.training_plan_supervisor_enabled {
                return Ok(None);
            }

            let model = settings
                .ai_agents
                .training_plan_supervisor_model
                .clone()
                .unwrap_or_else(|| DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL.to_string());
            let operation = TrainingPlanSupervisorOperation::pending(
                worker_operation_key,
                user_id,
                worker_saved_at_epoch_seconds,
                model,
                clock.now_epoch_seconds(),
            );
            repository.upsert(operation).await.map(Some)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        identity::Clock,
        settings::{
            AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings,
            IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, WahooConfig,
        },
        training_plan::{
            TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
            TrainingPlanReplacementResult, TrainingPlanSnapshot,
        },
    };

    use super::{
        NoopTrainingPlanSupervisorScheduler, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorScheduler, TrainingPlanSupervisorService,
    };
    use crate::domain::training_plan_supervisor::{
        TrainingPlanSupervisorDecision, TrainingPlanSupervisorStatus,
    };

    #[derive(Clone, Copy)]
    struct FixedClock {
        now_epoch_seconds: i64,
    }

    impl Clock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            self.now_epoch_seconds
        }
    }

    #[derive(Clone, Default)]
    struct InMemorySupervisorOperationRepository {
        stored: Arc<Mutex<Vec<TrainingPlanSupervisorOperation>>>,
    }

    impl TrainingPlanSupervisorOperationRepository for InMemorySupervisorOperationRepository {
        fn find_by_worker_operation_key(
            &self,
            worker_operation_key: &str,
        ) -> super::BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>
        {
            let stored = self.stored.clone();
            let worker_operation_key = worker_operation_key.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("supervisor operation repo mutex poisoned")
                    .iter()
                    .find(|operation| operation.worker_operation_key == worker_operation_key)
                    .cloned())
            })
        }

        fn upsert(
            &self,
            operation: TrainingPlanSupervisorOperation,
        ) -> super::BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
            let stored = self.stored.clone();
            Box::pin(async move {
                let mut stored = stored
                    .lock()
                    .expect("supervisor operation repo mutex poisoned");
                stored.retain(|existing| {
                    existing.worker_operation_key != operation.worker_operation_key
                });
                stored.push(operation.clone());
                Ok(operation)
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingProjectionRepository {
        stored: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
    }

    impl RecordingProjectionRepository {
        fn seed_day(&self, day: TrainingPlanProjectedDay) {
            self.stored
                .lock()
                .expect("projection repo mutex poisoned")
                .push(day);
        }

        fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
            self.stored
                .lock()
                .expect("projection repo mutex poisoned")
                .clone()
        }
    }

    impl TrainingPlanProjectionRepository for RecordingProjectionRepository {
        fn list_active_by_user_id(
            &self,
            _user_id: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            Box::pin(async {
                Err(TrainingPlanError::Repository(
                    "list_active_by_user_id not implemented in test".to_string(),
                ))
            })
        }

        fn find_active_by_operation_key(
            &self,
            _operation_key: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            Box::pin(async {
                Err(TrainingPlanError::Repository(
                    "find_active_by_operation_key not implemented in test".to_string(),
                ))
            })
        }

        fn find_active_by_user_id_and_operation_key(
            &self,
            _user_id: &str,
            _operation_key: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            Box::pin(async {
                Err(TrainingPlanError::Repository(
                    "find_active_by_user_id_and_operation_key not implemented in test".to_string(),
                ))
            })
        }

        fn replace_window(
            &self,
            _snapshot: TrainingPlanSnapshot,
            _projected_days: Vec<TrainingPlanProjectedDay>,
            _today: &str,
            _replaced_at_epoch_seconds: i64,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<TrainingPlanReplacementResult, TrainingPlanError>,
        > {
            Box::pin(async {
                Err(TrainingPlanError::Repository(
                    "replace_window not implemented in test".to_string(),
                ))
            })
        }

        fn update_supervisor_status(
            &self,
            user_id: &str,
            operation_key: &str,
            supervisor_status: Option<TrainingPlanSupervisorStatus>,
            updated_at_epoch_seconds: i64,
        ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let operation_key = operation_key.to_string();
            Box::pin(async move {
                for day in stored
                    .lock()
                    .expect("projection repo mutex poisoned")
                    .iter_mut()
                {
                    if day.user_id == user_id
                        && day.operation_key == operation_key
                        && day.superseded_at_epoch_seconds.is_none()
                    {
                        day.supervisor_status = supervisor_status;
                        day.updated_at_epoch_seconds = updated_at_epoch_seconds;
                    }
                }
                Ok(())
            })
        }
    }

    #[derive(Clone)]
    struct StubUserSettingsService {
        settings: Option<UserSettings>,
    }

    impl StubUserSettingsService {
        fn enabled(model: &str) -> Self {
            Self {
                settings: Some(UserSettings {
                    user_id: "user-1".to_string(),
                    ai_agents: AiAgentsConfig {
                        training_plan_supervisor_enabled: true,
                        training_plan_supervisor_model: Some(model.to_string()),
                        ..AiAgentsConfig::default()
                    },
                    intervals: IntervalsConfig::default(),
                    wahoo: WahooConfig::default(),
                    options: AnalysisOptions::default(),
                    availability: AvailabilitySettings::default(),
                    cycling: CyclingSettings::default(),
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                }),
            }
        }

        fn disabled() -> Self {
            Self {
                settings: Some(UserSettings {
                    user_id: "user-1".to_string(),
                    ai_agents: AiAgentsConfig::default(),
                    intervals: IntervalsConfig::default(),
                    wahoo: WahooConfig::default(),
                    options: AnalysisOptions::default(),
                    availability: AvailabilitySettings::default(),
                    cycling: CyclingSettings::default(),
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                }),
            }
        }

        fn no_settings() -> Self {
            Self { settings: None }
        }
    }

    impl UserSettingsUseCases for StubUserSettingsService {
        fn find_settings(
            &self,
            _user_id: &str,
        ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>>
        {
            let settings = self.settings.clone();
            Box::pin(async move { Ok(settings) })
        }

        fn get_settings(
            &self,
            _user_id: &str,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                settings.ok_or_else(|| SettingsError::Repository("missing settings".to_string()))
            })
        }

        fn update_ai_agents(
            &self,
            _user_id: &str,
            _ai_agents: AiAgentsConfig,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async {
                Err(SettingsError::Repository(
                    "update_ai_agents not implemented in test".to_string(),
                ))
            })
        }

        fn update_intervals(
            &self,
            _user_id: &str,
            _intervals: IntervalsConfig,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async {
                Err(SettingsError::Repository(
                    "update_intervals not implemented in test".to_string(),
                ))
            })
        }

        fn update_options(
            &self,
            _user_id: &str,
            _options: AnalysisOptions,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async {
                Err(SettingsError::Repository(
                    "update_options not implemented in test".to_string(),
                ))
            })
        }

        fn update_availability(
            &self,
            _user_id: &str,
            _availability: AvailabilitySettings,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async {
                Err(SettingsError::Repository(
                    "update_availability not implemented in test".to_string(),
                ))
            })
        }

        fn update_cycling(
            &self,
            _user_id: &str,
            _cycling: CyclingSettings,
        ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
            Box::pin(async {
                Err(SettingsError::Repository(
                    "update_cycling not implemented in test".to_string(),
                ))
            })
        }
    }

    #[tokio::test]
    async fn noop_scheduler_skips_pending_review() {
        let result = NoopTrainingPlanSupervisorScheduler
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap();

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn supervisor_service_skips_pending_review_when_disabled() {
        let service = TrainingPlanSupervisorService::new(
            InMemorySupervisorOperationRepository::default(),
            StubUserSettingsService::disabled(),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let result = service
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap();

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn supervisor_service_creates_pending_review_when_enabled() {
        let repository = InMemorySupervisorOperationRepository::default();
        let service = TrainingPlanSupervisorService::new(
            repository.clone(),
            StubUserSettingsService::enabled("gemini-2.5-pro"),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let result = service
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap()
            .expect("expected pending review");

        assert_eq!(result.worker_operation_key, "worker-op-1");
        assert_eq!(result.user_id, "user-1");
        assert_eq!(result.worker_saved_at_epoch_seconds, 1_700_000_000);
        assert_eq!(result.model, "gemini-2.5-pro");
        assert_eq!(result.status.as_str(), "pending");
        assert_eq!(result.created_at_epoch_seconds, 1_700_000_200);
        assert_eq!(result.updated_at_epoch_seconds, 1_700_000_200);

        let stored = repository
            .find_by_worker_operation_key("worker-op-1")
            .await
            .unwrap();
        assert_eq!(stored, Some(result));
    }

    #[tokio::test]
    async fn supervisor_service_reuses_existing_operation_for_same_worker_operation() {
        let repository = InMemorySupervisorOperationRepository::default();
        let service = TrainingPlanSupervisorService::new(
            repository.clone(),
            StubUserSettingsService::enabled("gemini-2.5-pro"),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let first = service
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap()
            .expect("expected first pending review");
        let second = service
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap()
            .expect("expected reused pending review");

        assert_eq!(second, first);
    }

    #[tokio::test]
    async fn supervisor_service_skips_pending_review_when_no_settings() {
        let service = TrainingPlanSupervisorService::new(
            InMemorySupervisorOperationRepository::default(),
            StubUserSettingsService::no_settings(),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let result = service
            .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000)
            .await
            .unwrap();

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn supervisor_service_completes_review_and_updates_active_projected_days() {
        let repository = InMemorySupervisorOperationRepository::default();
        repository
            .upsert(TrainingPlanSupervisorOperation::pending(
                "worker-op-1".to_string(),
                "user-1".to_string(),
                1_700_000_000,
                "gemini-2.5-pro".to_string(),
                1_700_000_100,
            ))
            .await
            .unwrap();
        let projections = RecordingProjectionRepository::default();
        projections.seed_day(TrainingPlanProjectedDay {
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            operation_key: "worker-op-1".to_string(),
            date: "2026-05-18".to_string(),
            rest_day: false,
            rest_day_reason: None,
            workout: None,
            supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        });
        projections.seed_day(TrainingPlanProjectedDay {
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            operation_key: "worker-op-1".to_string(),
            date: "2026-05-17".to_string(),
            rest_day: false,
            rest_day_reason: None,
            workout: None,
            supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
            superseded_at_epoch_seconds: Some(1_700_000_050),
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        });
        let service = TrainingPlanSupervisorService::new(
            repository.clone(),
            StubUserSettingsService::enabled("gemini-2.5-pro"),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let completed = service
            .complete_review(
                projections.clone(),
                "worker-op-1",
                TrainingPlanSupervisorReview {
                    decision: TrainingPlanSupervisorDecision::Accept,
                    reason: "plan already looks good".to_string(),
                    plan: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(completed.status, TrainingPlanSupervisorStatus::Accepted);
        assert_eq!(
            completed.review,
            Some(TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Accept,
                reason: "plan already looks good".to_string(),
                plan: None,
            })
        );
        assert_eq!(completed.updated_at_epoch_seconds, 1_700_000_200);

        let stored = repository
            .find_by_worker_operation_key("worker-op-1")
            .await
            .unwrap()
            .expect("expected stored operation");
        assert_eq!(stored, completed);

        let days = projections.stored_days();
        let active = days
            .iter()
            .find(|day| day.superseded_at_epoch_seconds.is_none())
            .expect("expected active day");
        assert_eq!(
            active.supervisor_status,
            Some(TrainingPlanSupervisorStatus::Accepted)
        );
        assert_eq!(active.updated_at_epoch_seconds, 1_700_000_200);

        let superseded = days
            .iter()
            .find(|day| day.superseded_at_epoch_seconds.is_some())
            .expect("expected superseded day");
        assert_eq!(
            superseded.supervisor_status,
            Some(TrainingPlanSupervisorStatus::Pending)
        );
        assert_eq!(superseded.updated_at_epoch_seconds, 1);
    }

    #[tokio::test]
    async fn supervisor_service_rejects_conflicting_second_terminal_review() {
        let repository = InMemorySupervisorOperationRepository::default();
        repository
            .upsert(
                TrainingPlanSupervisorOperation::pending(
                    "worker-op-1".to_string(),
                    "user-1".to_string(),
                    1_700_000_000,
                    "gemini-2.5-pro".to_string(),
                    1_700_000_100,
                )
                .complete_review(
                    TrainingPlanSupervisorReview {
                        decision: TrainingPlanSupervisorDecision::Accept,
                        reason: "looks good".to_string(),
                        plan: None,
                    },
                    1_700_000_150,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let service = TrainingPlanSupervisorService::new(
            repository,
            StubUserSettingsService::enabled("gemini-2.5-pro"),
            FixedClock {
                now_epoch_seconds: 1_700_000_200,
            },
        );

        let error = service
            .complete_review(
                RecordingProjectionRepository::default(),
                "worker-op-1",
                TrainingPlanSupervisorReview {
                    decision: TrainingPlanSupervisorDecision::Fail,
                    reason: "actually invalid".to_string(),
                    plan: None,
                },
            )
            .await
            .expect_err("expected conflicting terminal review to fail");

        assert_eq!(
            error,
            TrainingPlanError::Validation(
                "training plan supervisor review already completed with status accepted"
                    .to_string()
            )
        );
    }
}
