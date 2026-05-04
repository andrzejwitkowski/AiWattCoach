use crate::domain::{
    identity::Clock,
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, Weekday,
    },
};

#[derive(Clone)]
pub(crate) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_775_174_400
    }
}

#[derive(Clone)]
pub(crate) struct TestSettingsService;

impl UserSettingsUseCases for TestSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults("user-1".to_string(), 1);
            settings.cycling = CyclingSettings {
                full_name: Some("Alex".to_string()),
                ftp_watts: Some(300),
                athlete_prompt: Some("prefers concise coaching".to_string()),
                ..CyclingSettings::default()
            };
            settings.availability = test_availability();
            Ok(Some(settings))
        })
    }

    fn get_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults("user-1".to_string(), 1);
            settings.cycling = CyclingSettings {
                full_name: Some("Alex".to_string()),
                ftp_watts: Some(300),
                athlete_prompt: Some("prefers concise coaching".to_string()),
                ..CyclingSettings::default()
            };
            settings.availability = test_availability();
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        unreachable!()
    }
}

fn test_availability() -> AvailabilitySettings {
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
                available: true,
                max_duration_minutes: Some(180),
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    }
}
