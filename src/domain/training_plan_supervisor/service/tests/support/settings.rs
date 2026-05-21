use crate::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsUseCases, WahooConfig,
};

#[derive(Clone)]
pub(crate) struct StubUserSettingsService {
    settings: Option<UserSettings>,
}

impl StubUserSettingsService {
    pub(crate) fn enabled(model: &str) -> Self {
        Self {
            settings: Some(UserSettings {
                user_id: "user-1".to_string(),
                ai_agents: AiAgentsConfig {
                    gemini_api_key: Some("gem-key".to_string()),
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

    pub(crate) fn disabled() -> Self {
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

    pub(crate) fn no_settings() -> Self {
        Self { settings: None }
    }
}

impl UserSettingsUseCases for StubUserSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
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
