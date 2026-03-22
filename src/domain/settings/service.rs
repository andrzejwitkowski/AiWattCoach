use super::{
    AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError, UserSettings,
    UserSettingsRepository,
};
use crate::domain::identity::Clock;
use crate::domain::settings::ports::BoxFuture;

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
}

impl<Repo, Time> UserSettingsService<Repo, Time>
where
    Repo: UserSettingsRepository,
    Time: Clock,
{
    pub fn new(repository: Repo, clock: Time) -> Self {
        Self { repository, clock }
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
            service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_ai_agents(&user_id, ai_agents, now)
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
