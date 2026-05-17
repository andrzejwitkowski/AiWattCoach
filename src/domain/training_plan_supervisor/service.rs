use crate::domain::{
    identity::Clock,
    settings::{UserSettingsUseCases, DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL},
    training_plan::TrainingPlanError,
};

use super::{
    BoxFuture, TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
    TrainingPlanSupervisorScheduler,
};

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
        training_plan::TrainingPlanError,
    };

    use super::{
        NoopTrainingPlanSupervisorScheduler, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorScheduler,
        TrainingPlanSupervisorService,
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
}
