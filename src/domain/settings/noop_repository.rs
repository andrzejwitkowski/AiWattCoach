use super::{
    ports::{BoxFuture, WahooUserIdBackfillCandidate},
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsRepository,
};

#[derive(Clone, Default)]
pub struct NoopUserSettingsRepository;

impl UserSettingsRepository for NoopUserSettingsRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        Box::pin(async { Ok(None) })
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
        Box::pin(async move { Ok(settings) })
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
