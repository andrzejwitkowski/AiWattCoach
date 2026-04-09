use std::{future::Future, pin::Pin};

use aiwattcoach::domain::{
    intervals::IntervalsCredentials,
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases,
    },
};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Clone)]
pub(crate) struct FakeSettingsUseCases {
    settings: UserSettings,
}

impl FakeSettingsUseCases {
    pub(crate) fn with_intervals(intervals: IntervalsConfig) -> Self {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
        settings.intervals = intervals;
        Self { settings }
    }
}

impl UserSettingsUseCases for FakeSettingsUseCases {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(Some(settings)) })
    }

    fn get_settings(&self, _user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }
}

pub(crate) fn test_credentials() -> IntervalsCredentials {
    IntervalsCredentials {
        api_key: "secret-key".to_string(),
        athlete_id: "athlete-7".to_string(),
    }
}
