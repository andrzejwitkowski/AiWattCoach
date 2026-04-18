use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};

use crate::domain::{
    completed_workouts::CompletedWorkoutRepository, settings::UserSettingsRepository,
};

use super::{
    build_daily_training_load_snapshots, BoxFuture, FtpHistoryRepository,
    TrainingLoadDailySnapshotRepository, TrainingLoadDashboardPoint, TrainingLoadDashboardRange,
    TrainingLoadDashboardReport, TrainingLoadDashboardSummary, TrainingLoadError,
    TrainingLoadSnapshotRange, TrainingLoadTsbZone,
};

pub trait TrainingLoadRecomputeUseCases: Send + Sync {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), TrainingLoadError>>;
}

pub trait TrainingLoadDashboardReadUseCases: Send + Sync {
    fn build_report(
        &self,
        user_id: &str,
        range: TrainingLoadDashboardRange,
        today: &str,
    ) -> BoxFuture<Result<TrainingLoadDashboardReport, TrainingLoadError>>;
}

#[derive(Clone)]
pub struct TrainingLoadDashboardReadService<Snapshots>
where
    Snapshots: TrainingLoadDailySnapshotRepository,
{
    snapshots: Snapshots,
}

impl<Snapshots> TrainingLoadDashboardReadService<Snapshots>
where
    Snapshots: TrainingLoadDailySnapshotRepository,
{
    pub fn new(snapshots: Snapshots) -> Self {
        Self { snapshots }
    }
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
                return Err(TrainingLoadError::Repository(format!(
                    "invalid oldest_date '{oldest_date}' for training load recompute; expected format %Y-%m-%d"
                )));
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

impl<Snapshots> TrainingLoadDashboardReadUseCases for TrainingLoadDashboardReadService<Snapshots>
where
    Snapshots: TrainingLoadDailySnapshotRepository,
{
    fn build_report(
        &self,
        user_id: &str,
        range: TrainingLoadDashboardRange,
        today: &str,
    ) -> BoxFuture<Result<TrainingLoadDashboardReport, TrainingLoadError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let today = today.to_string();
        Box::pin(async move {
            let today = NaiveDate::parse_from_str(&today, "%Y-%m-%d").map_err(|_| {
                TrainingLoadError::Repository(format!(
                    "invalid dashboard report date '{today}'; expected format %Y-%m-%d"
                ))
            })?;
            let window_start = match range {
                TrainingLoadDashboardRange::Last90Days => {
                    (today - Duration::days(89)).format("%Y-%m-%d").to_string()
                }
                TrainingLoadDashboardRange::Season => format!("{:04}-01-01", today.year()),
                TrainingLoadDashboardRange::AllTime => service
                    .snapshots
                    .find_oldest_date_by_user_id(&user_id)
                    .await?
                    .unwrap_or_else(|| today.format("%Y-%m-%d").to_string()),
            };
            let window_end = today.format("%Y-%m-%d").to_string();
            let snapshots = service
                .snapshots
                .list_by_user_id_and_range(
                    &user_id,
                    &TrainingLoadSnapshotRange {
                        oldest: window_start.clone(),
                        newest: window_end.clone(),
                    },
                )
                .await?;
            let latest_snapshot = snapshots.last();
            let reference_ctl = snapshots
                .iter()
                .rev()
                .find(|snapshot| {
                    snapshot.date <= (today - Duration::days(14)).format("%Y-%m-%d").to_string()
                })
                .or_else(|| snapshots.first())
                .and_then(|snapshot| snapshot.ctl);

            Ok(TrainingLoadDashboardReport {
                range,
                window_start,
                window_end,
                has_training_load: !snapshots.is_empty(),
                summary: TrainingLoadDashboardSummary {
                    current_ctl: latest_snapshot.and_then(|snapshot| snapshot.ctl),
                    current_atl: latest_snapshot.and_then(|snapshot| snapshot.atl),
                    current_tsb: latest_snapshot.and_then(|snapshot| snapshot.tsb),
                    ftp_watts: latest_snapshot.and_then(|snapshot| snapshot.ftp_effective_watts),
                    average_if_28d: latest_snapshot.and_then(|snapshot| snapshot.average_if_28d),
                    average_ef_28d: latest_snapshot.and_then(|snapshot| snapshot.average_ef_28d),
                    load_delta_ctl_14d: latest_snapshot
                        .and_then(|snapshot| snapshot.ctl)
                        .zip(reference_ctl)
                        .map(|(current, previous)| round_to_2(current - previous)),
                    tsb_zone: classify_tsb_zone(latest_snapshot.and_then(|snapshot| snapshot.tsb)),
                },
                points: snapshots
                    .into_iter()
                    .map(|snapshot| TrainingLoadDashboardPoint {
                        date: snapshot.date,
                        daily_tss: snapshot.daily_tss,
                        ctl: snapshot.ctl,
                        atl: snapshot.atl,
                        tsb: snapshot.tsb,
                    })
                    .collect(),
            })
        })
    }
}

fn classify_tsb_zone(tsb: Option<f64>) -> TrainingLoadTsbZone {
    match tsb {
        Some(value) if value > 0.0 => TrainingLoadTsbZone::FreshnessPeak,
        Some(value) if value < -30.0 => TrainingLoadTsbZone::HighRisk,
        _ => TrainingLoadTsbZone::OptimalTraining,
    }
}

fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
