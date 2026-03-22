use std::{future::Future, pin::Pin};

use super::{
    AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError, UserSettings,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait UserSettingsRepository: Clone + Send + Sync + 'static {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>>;

    fn upsert(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>>;

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>>;

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>>;

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>>;

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>>;
}
