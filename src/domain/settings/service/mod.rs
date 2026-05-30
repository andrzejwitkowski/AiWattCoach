mod helpers;
mod updates;

#[cfg(test)]
mod tests;

use super::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsRepository,
};
use crate::domain::external_sync::{NoopProviderPollStateRepository, ProviderPollStateRepository};
use crate::domain::identity::Clock;
use crate::domain::llm::LlmContextCacheRepository;
use crate::domain::settings::ports::BoxFuture;
use crate::domain::training_load::{FtpHistoryRepository, TrainingLoadRecomputeUseCases};
use std::sync::Arc;

use helpers::FtpHistoryWritePort;

pub trait UserSettingsUseCases: Send + Sync {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>>;
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
    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
}

#[derive(Clone)]
pub struct UserSettingsService<Repo, Time, PollStates = NoopProviderPollStateRepository>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    repository: Repo,
    clock: Time,
    poll_states: PollStates,
    llm_context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    ftp_history_repository: Option<Arc<dyn FtpHistoryWritePort>>,
    training_load_recompute_service: Option<Arc<dyn TrainingLoadRecomputeUseCases>>,
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
            poll_states: NoopProviderPollStateRepository,
            llm_context_cache_repository: None,
            ftp_history_repository: None,
            training_load_recompute_service: None,
        }
    }
}

impl<Repo, Time, PollStates> UserSettingsService<Repo, Time, PollStates>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    pub fn with_provider_poll_states<NextPollStates>(
        self,
        poll_states: NextPollStates,
    ) -> UserSettingsService<Repo, Time, NextPollStates>
    where
        NextPollStates: ProviderPollStateRepository,
    {
        UserSettingsService {
            repository: self.repository,
            clock: self.clock,
            poll_states,
            llm_context_cache_repository: self.llm_context_cache_repository,
            ftp_history_repository: self.ftp_history_repository,
            training_load_recompute_service: self.training_load_recompute_service,
        }
    }

    pub fn with_llm_context_cache_repository(
        mut self,
        llm_context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.llm_context_cache_repository = Some(llm_context_cache_repository);
        self
    }

    pub fn with_ftp_history_repository(
        mut self,
        ftp_history_repository: impl FtpHistoryRepository,
    ) -> Self {
        self.ftp_history_repository = Some(Arc::new(ftp_history_repository));
        self
    }

    pub fn with_training_load_recompute_service(
        mut self,
        training_load_recompute_service: Arc<dyn TrainingLoadRecomputeUseCases>,
    ) -> Self {
        self.training_load_recompute_service = Some(training_load_recompute_service);
        self
    }

    pub(super) async fn get_or_create(&self, user_id: &str) -> Result<UserSettings, SettingsError> {
        if let Some(settings) = self.repository.find_by_user_id(user_id).await? {
            return Ok(settings);
        }

        let now = self.clock.now_epoch_seconds();
        let defaults = UserSettings::new_defaults(user_id.to_string(), now);
        self.repository.upsert(defaults).await
    }
}

impl<Repo, Time, PollStates> UserSettingsUseCases for UserSettingsService<Repo, Time, PollStates>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    fn find_settings(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { repository.find_by_user_id(&user_id).await })
    }

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
        Box::pin(async move { service.update_ai_agents_impl(&user_id, ai_agents).await })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_intervals_impl(&user_id, intervals).await })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_options_impl(&user_id, options).await })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_cycling_impl(&user_id, cycling).await })
    }

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service
                .update_availability_impl(&user_id, availability)
                .await
        })
    }
}
