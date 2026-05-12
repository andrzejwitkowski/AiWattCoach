use std::sync::{Arc, Mutex};

use crate::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, BoxFuture, CyclingSettings,
    IntervalsConfig, SettingsError, UserSettings, UserSettingsRepository,
    WahooUserIdBackfillCandidate,
};

#[derive(Clone)]
pub struct InMemoryUserSettingsRepository {
    settings: Arc<Mutex<Option<UserSettings>>>,
}

impl InMemoryUserSettingsRepository {
    pub fn with_ftp(ftp_watts: u32) -> Self {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_700_000_000);
        settings.cycling.ftp_watts = Some(ftp_watts);
        Self {
            settings: Arc::new(Mutex::new(Some(settings))),
        }
    }
}

impl UserSettingsRepository for InMemoryUserSettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self
            .settings
            .lock()
            .expect("settings mutex poisoned")
            .clone();
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(settings.filter(|s| s.user_id == user_id)) })
    }

    fn find_by_wahoo_user_id(
        &self,
        _wahoo_user_id: i64,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> BoxFuture<Result<Vec<WahooUserIdBackfillCandidate>, SettingsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let state = self.settings.clone();
        Box::pin(async move {
            *state.lock().expect("settings mutex poisoned") = Some(settings.clone());
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }
}
