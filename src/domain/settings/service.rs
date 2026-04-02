use super::{
    AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError, UserSettings,
    UserSettingsRepository,
};
use crate::domain::identity::Clock;
use crate::domain::llm::LlmContextCacheRepository;
use crate::domain::settings::ports::BoxFuture;
use std::sync::Arc;

pub trait UserSettingsUseCases: Send + Sync {
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
}

#[derive(Clone)]
pub struct UserSettingsService<Repo, Time>
where
    Repo: UserSettingsRepository,
    Time: Clock,
{
    repository: Repo,
    clock: Time,
    llm_context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
}

impl<Repo, Time> UserSettingsService<Repo, Time>
where
    Repo: UserSettingsRepository,
    Time: Clock,
{
    pub fn new(repository: Repo, clock: Time) -> Self {
        Self {
            repository,
            clock,
            llm_context_cache_repository: None,
        }
    }

    pub fn with_llm_context_cache_repository(
        mut self,
        llm_context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.llm_context_cache_repository = Some(llm_context_cache_repository);
        self
    }

    async fn get_or_create(&self, user_id: &str) -> Result<UserSettings, SettingsError> {
        if let Some(settings) = self.repository.find_by_user_id(user_id).await? {
            return Ok(settings);
        }
        let now = self.clock.now_epoch_seconds();
        let defaults = UserSettings::new_defaults(user_id.to_string(), now);
        self.repository.upsert(defaults).await
    }
}

impl<Repo, Time> UserSettingsUseCases for UserSettingsService<Repo, Time>
where
    Repo: UserSettingsRepository,
    Time: Clock,
{
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_or_create(&user_id).await })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let previous = service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_ai_agents(&user_id, ai_agents, now)
                .await?;
            let updated = service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })?;

            if should_invalidate_llm_cache(&previous.ai_agents, &updated.ai_agents) {
                if let Some(repository) = &service.llm_context_cache_repository {
                    if let Err(error) = repository.delete_by_user_id(&user_id).await {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "failed to invalidate llm context cache after settings update"
                        );
                    }
                }
            }

            Ok(updated)
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_intervals(&user_id, intervals, now)
                .await?;
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_options(&user_id, options, now)
                .await?;
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_cycling(&user_id, cycling, now)
                .await?;
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }
}

fn should_invalidate_llm_cache(previous: &AiAgentsConfig, updated: &AiAgentsConfig) -> bool {
    previous.selected_provider != updated.selected_provider
        || previous.selected_model != updated.selected_model
        || previous.openai_api_key != updated.openai_api_key
        || previous.gemini_api_key != updated.gemini_api_key
        || previous.openrouter_api_key != updated.openrouter_api_key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        identity::Clock,
        llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestClock;

    impl Clock for TestClock {
        fn now_epoch_seconds(&self) -> i64 {
            1_700_000_000
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryUserSettingsRepository {
        settings: Arc<Mutex<Option<UserSettings>>>,
    }

    impl InMemoryUserSettingsRepository {
        fn with_settings(settings: UserSettings) -> Self {
            Self {
                settings: Arc::new(Mutex::new(Some(settings))),
            }
        }
    }

    impl UserSettingsRepository for InMemoryUserSettingsRepository {
        fn find_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move { Ok(settings.lock().unwrap().clone()) })
        }

        fn upsert(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>> {
            let store = self.settings.clone();
            Box::pin(async move {
                *store.lock().unwrap() = Some(settings.clone());
                Ok(settings)
            })
        }

        fn update_ai_agents(
            &self,
            _user_id: &str,
            ai_agents: AiAgentsConfig,
            updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                let mut guard = settings.lock().unwrap();
                let current = guard
                    .as_mut()
                    .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
                current.ai_agents = ai_agents;
                current.updated_at_epoch_seconds = updated_at_epoch_seconds;
                Ok(())
            })
        }

        fn update_intervals(
            &self,
            _user_id: &str,
            _intervals: IntervalsConfig,
            _updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            Box::pin(async move { unreachable!("not used in test") })
        }

        fn update_options(
            &self,
            _user_id: &str,
            _options: AnalysisOptions,
            _updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            Box::pin(async move { unreachable!("not used in test") })
        }

        fn update_cycling(
            &self,
            _user_id: &str,
            _cycling: CyclingSettings,
            _updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            Box::pin(async move { unreachable!("not used in test") })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingCacheRepository {
        deleted_users: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingCacheRepository {
        fn deleted_users(&self) -> Vec<String> {
            self.deleted_users.lock().unwrap().clone()
        }
    }

    impl LlmContextCacheRepository for RecordingCacheRepository {
        fn find_reusable(
            &self,
            _user_id: &str,
            _provider: &crate::domain::llm::LlmProvider,
            _model: &str,
            _scope_key: &str,
            _context_hash: &str,
            _now_epoch_seconds: i64,
        ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
            Box::pin(async move { unreachable!("not used in test") })
        }

        fn upsert(
            &self,
            _cache: LlmContextCache,
        ) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
            Box::pin(async move { unreachable!("not used in test") })
        }

        fn delete_by_user_id(&self, user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
            let deleted_users = self.deleted_users.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                deleted_users.lock().unwrap().push(user_id);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn update_ai_agents_invalidates_llm_cache_when_provider_config_changes() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::OpenAi);
        settings.ai_agents.selected_model = Some("gpt-4o-mini".to_string());
        settings.ai_agents.openai_api_key = Some("sk-old".to_string());

        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let cache_repository = Arc::new(RecordingCacheRepository::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_llm_context_cache_repository(cache_repository.clone());

        let updated = service
            .update_ai_agents(
                "user-1",
                AiAgentsConfig {
                    selected_provider: Some(crate::domain::llm::LlmProvider::OpenRouter),
                    selected_model: Some("openai/gpt-4o-mini".to_string()),
                    openai_api_key: Some("sk-old".to_string()),
                    openrouter_api_key: Some("or-new".to_string()),
                    ..AiAgentsConfig::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(
            updated.ai_agents.selected_model.as_deref(),
            Some("openai/gpt-4o-mini")
        );
        assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
    }

    #[tokio::test]
    async fn update_ai_agents_skips_llm_cache_invalidation_when_provider_config_is_unchanged() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::Gemini);
        settings.ai_agents.selected_model = Some("gemini-2.5-flash".to_string());
        settings.ai_agents.gemini_api_key = Some("gem-key".to_string());

        let repository = InMemoryUserSettingsRepository::with_settings(settings.clone());
        let cache_repository = Arc::new(RecordingCacheRepository::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_llm_context_cache_repository(cache_repository.clone());

        service
            .update_ai_agents("user-1", settings.ai_agents)
            .await
            .unwrap();

        assert!(cache_repository.deleted_users().is_empty());
    }
}
