use std::sync::Arc;

use chrono::{Duration, NaiveDate};

use crate::domain::{
    training_context::TrainingContextBuilder,
    training_load::{
        FtpHistoryEntry, FtpHistoryRepository, FtpSource, InMemoryFtpHistoryRepository,
        InMemoryTrainingLoadDailySnapshotRepository, TrainingLoadDailySnapshot,
        TrainingLoadDailySnapshotRepository,
    },
};

use super::{
    super::super::DefaultTrainingContextBuilder,
    super::support::{
        sample_completed_workout_on_date_with_ftp, DirectCompletedWorkoutTargetService, FixedClock,
        TestCompletedWorkoutRepository, TestPlannedWorkoutRepository, TestSettingsService,
        TestSpecialDayRepository, TestWorkoutSummaryRepository,
    },
};

#[tokio::test]
async fn builder_uses_training_load_snapshots_for_historical_aggregates_and_trend() {
    let snapshot_repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    seed_snapshot_trend(&snapshot_repository).await;

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_load_daily_snapshot_repository(snapshot_repository);

    let result = builder.build("user-1", "ride-1").await.unwrap();
    let last_point = result.context.history.load_trend.last().unwrap();

    assert_eq!(result.context.history.total_tss, 328);
    assert_eq!(result.context.history.ctl, Some(55.5));
    assert_eq!(result.context.history.atl, Some(66.6));
    assert_eq!(result.context.history.tsb, Some(-11.1));
    assert_eq!(result.context.history.average_tss_7d, Some(70.7));
    assert_eq!(result.context.history.average_tss_28d, Some(44.4));
    assert_eq!(result.context.history.average_if_28d, Some(0.91));
    assert_eq!(result.context.history.average_ef_28d, Some(1.47));
    assert_eq!(result.context.history.load_trend.len(), 42);
    assert_eq!(last_point.date, "2026-04-03");
    assert_eq!(last_point.period_tss, 123);
    assert_eq!(last_point.rolling_tss_7d, Some(70.7));
    assert_eq!(last_point.rolling_tss_28d, Some(44.4));
    assert_eq!(last_point.ctl, Some(55.5));
    assert_eq!(last_point.atl, Some(66.6));
    assert_eq!(last_point.tsb, Some(-11.1));
}

#[tokio::test]
async fn builder_uses_ftp_history_for_chronological_ftp_change() {
    let snapshot_repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    seed_snapshot_trend(&snapshot_repository).await;

    let ftp_history_repository = InMemoryFtpHistoryRepository::default();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-03-01".to_string(),
            ftp_watts: 280,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        },
    )
    .await
    .unwrap();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-02".to_string(),
            ftp_watts: 305,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 2,
            updated_at_epoch_seconds: 2,
        },
    )
    .await
    .unwrap();

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-late",
            "2026-04-03T08:00:00",
            Some(300),
            Some("intervals-event:101".to_string()),
        ),
        sample_completed_workout_on_date_with_ftp(
            "ride-early",
            "2026-03-15T08:00:00",
            Some(350),
            None,
        ),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_load_daily_snapshot_repository(snapshot_repository)
    .with_ftp_history_repository(ftp_history_repository);

    let result = builder.build("user-1", "ride-late").await.unwrap();

    assert_eq!(result.context.history.ftp_current, Some(305));
    assert_eq!(result.context.history.ftp_change, Some(25));
}

#[tokio::test]
async fn builder_ignores_ftp_history_entries_after_focus_date() {
    let ftp_history_repository = InMemoryFtpHistoryRepository::default();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-03-01".to_string(),
            ftp_watts: 280,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        },
    )
    .await
    .unwrap();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-02".to_string(),
            ftp_watts: 305,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 2,
            updated_at_epoch_seconds: 2,
        },
    )
    .await
    .unwrap();

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp("ride-older", "2026-03-20T08:00:00", None, None),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_ftp_history_repository(ftp_history_repository);

    let result = builder.build("user-1", "ride-older").await.unwrap();

    assert_eq!(result.context.history.window_end, "2026-03-20");
    assert_eq!(result.context.history.ftp_current, Some(280));
    assert_eq!(result.context.history.ftp_change, Some(0));
}

#[tokio::test]
async fn builder_ignores_partial_snapshot_windows_for_historical_aggregates() {
    let snapshot_repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    TrainingLoadDailySnapshotRepository::upsert(
        &snapshot_repository,
        TrainingLoadDailySnapshot {
            user_id: "user-1".to_string(),
            date: "2026-04-03".to_string(),
            daily_tss: Some(123),
            rolling_tss_7d: Some(70.7),
            rolling_tss_28d: Some(44.4),
            ctl: Some(55.5),
            atl: Some(66.6),
            tsb: Some(-11.1),
            average_if_28d: Some(0.91),
            average_ef_28d: Some(1.47),
            ftp_effective_watts: Some(305),
            ftp_source: Some(FtpSource::Settings),
            recomputed_at_epoch_seconds: 10,
            created_at_epoch_seconds: 10,
            updated_at_epoch_seconds: 10,
        },
    )
    .await
    .unwrap();

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_load_daily_snapshot_repository(snapshot_repository);

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.history.total_tss, 80);
    assert_eq!(result.context.history.ctl, Some(1.9));
    assert_eq!(result.context.history.atl, Some(11.43));
    assert_eq!(result.context.history.tsb, Some(-9.53));
    assert_eq!(result.context.history.average_tss_7d, Some(11.43));
    assert_eq!(result.context.history.average_tss_28d, Some(2.86));
    assert_eq!(result.context.history.average_if_28d, Some(0.83));
    assert_eq!(result.context.history.average_ef_28d, Some(1.2));
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .map(|point| point.period_tss),
        Some(80)
    );
}

#[tokio::test]
async fn builder_ignores_snapshot_windows_with_missing_days_inside_range() {
    let snapshot_repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    seed_snapshot_trend_with_gap(
        &snapshot_repository,
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
    )
    .await;

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_load_daily_snapshot_repository(snapshot_repository);

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.history.total_tss, 80);
    assert_eq!(result.context.history.ctl, Some(1.9));
    assert_eq!(result.context.history.atl, Some(11.43));
    assert_eq!(result.context.history.tsb, Some(-9.53));
    assert_eq!(result.context.history.average_tss_7d, Some(11.43));
    assert_eq!(result.context.history.average_tss_28d, Some(2.86));
    assert_eq!(result.context.history.average_if_28d, Some(0.83));
    assert_eq!(result.context.history.average_ef_28d, Some(1.2));
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .map(|point| point.period_tss),
        Some(80)
    );
}

#[tokio::test]
async fn builder_ignores_snapshot_ftp_when_snapshot_window_is_incomplete() {
    let snapshot_repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    seed_snapshot_trend_with_gap(
        &snapshot_repository,
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
    )
    .await;

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_load_daily_snapshot_repository(snapshot_repository);

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.history.ftp_current, Some(300));
    assert_eq!(result.context.history.ftp_change, Some(0));
}

#[tokio::test]
async fn builder_falls_back_to_activity_ftp_after_history_clear() {
    let ftp_history_repository = InMemoryFtpHistoryRepository::default();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-03-01".to_string(),
            ftp_watts: 280,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        },
    )
    .await
    .unwrap();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-02".to_string(),
            ftp_watts: 0,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 2,
            updated_at_epoch_seconds: 2,
        },
    )
    .await
    .unwrap();

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-cleared",
            "2026-04-03T08:00:00",
            Some(300),
            None,
        ),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_ftp_history_repository(ftp_history_repository);

    let result = builder.build("user-1", "ride-cleared").await.unwrap();

    assert_eq!(result.context.history.ftp_current, Some(300));
    assert_eq!(result.context.history.ftp_change, Some(0));
}

async fn seed_snapshot_trend(repository: &InMemoryTrainingLoadDailySnapshotRepository) {
    seed_snapshot_trend_with_gap(repository, NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()).await;
}

async fn seed_snapshot_trend_with_gap(
    repository: &InMemoryTrainingLoadDailySnapshotRepository,
    missing_date: NaiveDate,
) {
    let start = NaiveDate::from_ymd_opt(2025, 6, 20).unwrap();
    let trend_start = NaiveDate::from_ymd_opt(2026, 2, 21).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 4, 3).unwrap();

    for offset in 0..=(end - start).num_days() {
        let current = start + Duration::days(offset);
        if current == missing_date {
            continue;
        }
        let is_trend_window = current >= trend_start;
        let is_last = current == end;
        let date = current.format("%Y-%m-%d").to_string();
        TrainingLoadDailySnapshotRepository::upsert(
            repository,
            TrainingLoadDailySnapshot {
                user_id: "user-1".to_string(),
                date,
                daily_tss: Some(if is_last {
                    123
                } else if is_trend_window {
                    5
                } else {
                    0
                }),
                rolling_tss_7d: Some(if is_last {
                    70.7
                } else if is_trend_window {
                    5.0
                } else {
                    0.0
                }),
                rolling_tss_28d: Some(if is_last {
                    44.4
                } else if is_trend_window {
                    5.0
                } else {
                    0.0
                }),
                ctl: Some(if is_last {
                    55.5
                } else if is_trend_window {
                    1.0
                } else {
                    0.0
                }),
                atl: Some(if is_last {
                    66.6
                } else if is_trend_window {
                    1.0
                } else {
                    0.0
                }),
                tsb: Some(if is_last { -11.1 } else { 0.0 }),
                average_if_28d: Some(if is_last {
                    0.91
                } else if is_trend_window {
                    0.75
                } else {
                    0.0
                }),
                average_ef_28d: Some(if is_last {
                    1.47
                } else if is_trend_window {
                    1.1
                } else {
                    0.0
                }),
                ftp_effective_watts: Some(if is_last { 305 } else { 280 }),
                ftp_source: Some(FtpSource::Settings),
                recomputed_at_epoch_seconds: 10,
                created_at_epoch_seconds: 10,
                updated_at_epoch_seconds: 10,
            },
        )
        .await
        .unwrap();
    }
}
