use std::sync::Mutex;

use aiwattcoach::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsUseCases,
};

use super::app::BoxFuture;

pub(crate) struct TestSettingsService {
    settings: Mutex<Option<UserSettings>>,
}

impl TestSettingsService {
    pub(crate) fn new() -> Self {
        Self {
            settings: Mutex::new(None),
        }
    }

    pub(crate) fn with_settings(settings: UserSettings) -> Self {
        Self {
            settings: Mutex::new(Some(settings)),
        }
    }

    fn take_or_default_settings(&self, user_id: &str) -> UserSettings {
        self.settings
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(|| UserSettings::new_defaults(user_id.to_string(), 1000))
    }

    fn store_updated_settings(&self, settings: UserSettings) -> UserSettings {
        let result = settings.clone();
        *self.settings.lock().unwrap() = Some(settings);
        result
    }
}

impl Default for TestSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserSettingsUseCases for TestSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = { self.settings.lock().unwrap().clone() };
        Box::pin(async move { Ok(settings) })
    }

    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        let settings = { self.settings.lock().unwrap().clone() };
        Box::pin(async move {
            Ok(settings.unwrap_or_else(|| UserSettings::new_defaults(user_id, 1000)))
        })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let mut settings = self.take_or_default_settings(user_id);
        settings.ai_agents = ai_agents;
        settings.updated_at_epoch_seconds = 2000;
        let result = self.store_updated_settings(settings);
        Box::pin(async move { Ok(result) })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let mut settings = self.take_or_default_settings(user_id);
        settings.intervals = IntervalsConfig {
            connected: intervals.api_key.is_some() && intervals.athlete_id.is_some(),
            ..intervals
        };
        settings.updated_at_epoch_seconds = 2000;
        let result = self.store_updated_settings(settings);
        Box::pin(async move { Ok(result) })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let mut settings = self.take_or_default_settings(user_id);
        settings.options = options;
        settings.updated_at_epoch_seconds = 2000;
        let result = self.store_updated_settings(settings);
        Box::pin(async move { Ok(result) })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let mut settings = self.take_or_default_settings(user_id);
        settings.cycling = cycling;
        settings.updated_at_epoch_seconds = 2000;
        let result = self.store_updated_settings(settings);
        Box::pin(async move { Ok(result) })
    }

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let mut settings = self.take_or_default_settings(user_id);
        settings.availability = availability;
        settings.updated_at_epoch_seconds = 2000;
        let result = self.store_updated_settings(settings);
        Box::pin(async move { Ok(result) })
    }
}

pub(crate) struct RepositoryErrorSettingsService {
    message: String,
}

impl RepositoryErrorSettingsService {
    pub(crate) fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl UserSettingsUseCases for RepositoryErrorSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn get_settings(&self, _user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let message = self.message.clone();
        Box::pin(async move { Err(SettingsError::Repository(message)) })
    }
}
