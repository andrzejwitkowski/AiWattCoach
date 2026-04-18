use chrono::{DateTime, Duration, Utc};

use crate::domain::{
    completed_workouts::CompletedWorkoutRepository, settings::UserSettingsRepository,
};

use super::{
    build_daily_training_load_snapshots, BoxFuture, FtpHistoryRepository,
    TrainingLoadDailySnapshotRepository, TrainingLoadError, TrainingLoadSnapshotRange,
};

pub trait TrainingLoadRecomputeUseCases: Send + Sync {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), TrainingLoadError>>;
}

#[derive(Clone)]
pub struct TrainingLoadRecomputeService<Workouts, FtpHistory, Snapshots, Settings>
where
    Workouts: CompletedWorkoutRepository,
    FtpHistory: FtpHistoryRepository,
    Snapshots: TrainingLoadDailySnapshotRepository,
    Settings: UserSettingsRepository,
{
    completed_workouts: Workouts,
    ftp_history: FtpHistory,
    snapshots: Snapshots,
    settings_repository: Settings,
    warmup_days: i64,
}

impl<Workouts, FtpHistory, Snapshots, Settings>
    TrainingLoadRecomputeService<Workouts, FtpHistory, Snapshots, Settings>
where
    Workouts: CompletedWorkoutRepository,
    FtpHistory: FtpHistoryRepository,
    Snapshots: TrainingLoadDailySnapshotRepository,
    Settings: UserSettingsRepository,
{
    pub fn new(
        completed_workouts: Workouts,
        ftp_history: FtpHistory,
        snapshots: Snapshots,
        settings_repository: Settings,
    ) -> Self {
        Self {
            completed_workouts,
            ftp_history,
            snapshots,
            settings_repository,
            warmup_days: 120,
        }
    }

    #[cfg(test)]
    pub fn with_warmup_days(mut self, warmup_days: i64) -> Self {
        self.warmup_days = warmup_days;
        self
    }
}

impl<Workouts, FtpHistory, Snapshots, Settings> TrainingLoadRecomputeUseCases
    for TrainingLoadRecomputeService<Workouts, FtpHistory, Snapshots, Settings>
where
    Workouts: CompletedWorkoutRepository,
    FtpHistory: FtpHistoryRepository,
    Snapshots: TrainingLoadDailySnapshotRepository,
    Settings: UserSettingsRepository,
{
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), TrainingLoadError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            let settings = service
                .settings_repository
                .find_by_user_id(&user_id)
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    TrainingLoadError::Repository(format!(
                        "settings missing for training load recompute user '{user_id}'"
                    ))
                })?;
            let app_entry_date =
                DateTime::<Utc>::from_timestamp(settings.created_at_epoch_seconds, 0)
                    .map(|value| value.date_naive())
                    .unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH.date_naive());
            let Some(oldest_date) =
                chrono::NaiveDate::parse_from_str(&oldest_date, "%Y-%m-%d").ok()
            else {
                return Ok(());
            };
            let warmup_start = oldest_date - Duration::days(service.warmup_days);
            let newest = DateTime::<Utc>::from_timestamp(now_epoch_seconds, 0)
                .map(|value| value.date_naive())
                .unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH.date_naive());
            let range = TrainingLoadSnapshotRange {
                oldest: warmup_start.format("%Y-%m-%d").to_string(),
                newest: newest.format("%Y-%m-%d").to_string(),
            };
            let workouts = service
                .completed_workouts
                .list_by_user_id_and_date_range(&user_id, &range.oldest, &range.newest)
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;
            let ftp_history = service.ftp_history.list_by_user_id(&user_id).await?;
            let snapshots = build_daily_training_load_snapshots(
                &user_id,
                &range,
                &workouts,
                &ftp_history,
                &app_entry_date.format("%Y-%m-%d").to_string(),
                now_epoch_seconds,
            );

            for snapshot in snapshots {
                service.snapshots.upsert(snapshot).await?;
            }

            Ok(())
        })
    }
}
