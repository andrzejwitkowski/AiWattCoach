use std::sync::Arc;

use aiwattcoach::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
    IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, Weekday,
};

#[derive(Clone)]
pub(crate) struct TestAvailabilitySettingsService {
    configured: bool,
}

impl Default for TestAvailabilitySettingsService {
    fn default() -> Self {
        Self { configured: true }
    }
}

impl TestAvailabilitySettingsService {
    pub(crate) fn unconfigured() -> Arc<dyn UserSettingsUseCases> {
        Arc::new(Self { configured: false })
    }
}

impl UserSettingsUseCases for TestAvailabilitySettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = configured_availability();
            }
            Ok(Some(settings))
        })
    }

    fn get_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        let configured = self.configured;
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1_700_000_000);
            if configured {
                settings.availability = configured_availability();
            }
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
}

fn configured_availability() -> AvailabilitySettings {
    AvailabilitySettings {
        configured: true,
        days: vec![
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: true,
                max_duration_minutes: Some(120),
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    }
}
