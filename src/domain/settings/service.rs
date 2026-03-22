use super::{BoxFuture, SettingsError, UserSettings, UserSettingsRepository};

pub trait UserSettingsUseCases: Send + Sync {
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<Option<UserSettings>, SettingsError>>;
    fn save_settings(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>>;
}

#[derive(Clone)]
pub struct UserSettingsService<Repo> {
    repo: Repo,
}

impl<Repo> UserSettingsService<Repo>
where
    Repo: UserSettingsRepository,
{
    pub fn new(repo: Repo) -> Self {
        Self { repo }
    }

    pub async fn get_settings(&self, user_id: &str) -> Result<Option<UserSettings>, SettingsError> {
        self.repo.find_by_user_id(user_id).await
    }

    pub async fn save_settings(&self, settings: UserSettings) -> Result<UserSettings, SettingsError> {
        self.repo.save(settings).await
    }
}

impl<Repo> UserSettingsUseCases for UserSettingsService<Repo>
where
    Repo: UserSettingsRepository,
{
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_settings(&user_id).await })
    }

    fn save_settings(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        Box::pin(async move { service.save_settings(settings).await })
    }
}
