use super::{
    FtpHistoryEntry, FtpSource, TrainingLoadDailySnapshot, TrainingLoadDashboardRange,
    TrainingLoadDashboardReadService, TrainingLoadDashboardReadUseCases, TrainingLoadError,
    TrainingLoadRecomputeService, TrainingLoadRecomputeUseCases, TrainingLoadSnapshotRange,
    TrainingLoadTsbZone,
};
use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutInterval,
        CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics, CompletedWorkoutRepository,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsRepository,
    },
    training_load::{
        build_daily_training_load_snapshots, FtpHistoryRepository, InMemoryFtpHistoryRepository,
        InMemoryTrainingLoadDailySnapshotRepository, TrainingLoadDailySnapshotRepository,
    },
};

#[derive(Clone)]
struct StaticSettingsRepository {
    settings: UserSettings,
}

#[derive(Clone, Default)]
struct EmptyCompletedWorkoutRepository;

impl CompletedWorkoutRepository for EmptyCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        _user_id: &str,
        _source_activity_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_latest_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async move { Ok(workout) })
    }
}

impl UserSettingsRepository for StaticSettingsRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(Some(settings)) })
    }

    fn upsert(
        &self,
        settings: UserSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async move { Ok(settings) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        unreachable!()
    }
}

#[test]
fn ftp_history_entry_keeps_effective_ftp_metadata() {
    let entry = FtpHistoryEntry {
        user_id: "user-1".to_string(),
        effective_from_date: "2026-04-17".to_string(),
        ftp_watts: 285,
        source: FtpSource::Settings,
        created_at_epoch_seconds: 100,
        updated_at_epoch_seconds: 200,
    };

    assert_eq!(entry.user_id, "user-1");
    assert_eq!(entry.effective_from_date, "2026-04-17");
    assert_eq!(entry.ftp_watts, 285);
    assert_eq!(entry.source, FtpSource::Settings);
    assert_eq!(entry.created_at_epoch_seconds, 100);
    assert_eq!(entry.updated_at_epoch_seconds, 200);
}

#[test]
fn training_load_daily_snapshot_keeps_projected_metrics() {
    let snapshot = TrainingLoadDailySnapshot {
        user_id: "user-1".to_string(),
        date: "2026-04-17".to_string(),
        daily_tss: Some(78),
        rolling_tss_7d: Some(55.5),
        rolling_tss_28d: Some(49.1),
        ctl: Some(61.2),
        atl: Some(74.4),
        tsb: Some(-13.2),
        average_if_28d: Some(0.86),
        average_ef_28d: Some(1.34),
        ftp_effective_watts: Some(285),
        ftp_source: Some(FtpSource::Settings),
        recomputed_at_epoch_seconds: 300,
        created_at_epoch_seconds: 100,
        updated_at_epoch_seconds: 300,
    };

    assert_eq!(snapshot.user_id, "user-1");
    assert_eq!(snapshot.date, "2026-04-17");
    assert_eq!(snapshot.daily_tss, Some(78));
    assert_eq!(snapshot.rolling_tss_7d, Some(55.5));
    assert_eq!(snapshot.rolling_tss_28d, Some(49.1));
    assert_eq!(snapshot.ctl, Some(61.2));
    assert_eq!(snapshot.atl, Some(74.4));
    assert_eq!(snapshot.tsb, Some(-13.2));
    assert_eq!(snapshot.average_if_28d, Some(0.86));
    assert_eq!(snapshot.average_ef_28d, Some(1.34));
    assert_eq!(snapshot.ftp_effective_watts, Some(285));
    assert_eq!(snapshot.ftp_source, Some(FtpSource::Settings));
    assert_eq!(snapshot.recomputed_at_epoch_seconds, 300);
    assert_eq!(snapshot.created_at_epoch_seconds, 100);
    assert_eq!(snapshot.updated_at_epoch_seconds, 300);
}

#[test]
fn training_load_error_formats_repository_message() {
    let error = TrainingLoadError::Repository("repo exploded".to_string());

    assert_eq!(error.to_string(), "repo exploded");
}

#[test]
fn training_load_snapshot_range_uses_inclusive_boundaries() {
    let range = TrainingLoadSnapshotRange {
        oldest: "2026-04-01".to_string(),
        newest: "2026-04-30".to_string(),
    };

    assert_eq!(range.oldest, "2026-04-01");
    assert_eq!(range.newest, "2026-04-30");
}

#[test]
fn in_memory_training_load_daily_snapshot_repository_finds_oldest_date_for_user() {
    let repository = InMemoryTrainingLoadDailySnapshotRepository::default();

    futures::executor::block_on(async {
        repository
            .upsert(sample_snapshot_with_values(
                "user-1",
                "2026-04-05",
                SnapshotValues {
                    daily_tss: Some(40),
                    ctl: Some(30.0),
                    atl: Some(45.0),
                    tsb: Some(-15.0),
                    ftp_effective_watts: Some(280),
                    average_if_28d: Some(0.82),
                    average_ef_28d: Some(1.25),
                },
            ))
            .await
            .unwrap();
        repository
            .upsert(sample_snapshot_with_values(
                "user-1",
                "2026-04-01",
                SnapshotValues {
                    daily_tss: Some(50),
                    ctl: Some(28.0),
                    atl: Some(50.0),
                    tsb: Some(-22.0),
                    ftp_effective_watts: Some(280),
                    average_if_28d: None,
                    average_ef_28d: None,
                },
            ))
            .await
            .unwrap();
        repository
            .upsert(sample_snapshot_with_values(
                "user-2",
                "2026-03-20",
                SnapshotValues {
                    daily_tss: Some(60),
                    ctl: Some(40.0),
                    atl: Some(55.0),
                    tsb: Some(-15.0),
                    ftp_effective_watts: Some(300),
                    average_if_28d: Some(0.91),
                    average_ef_28d: Some(1.31),
                },
            ))
            .await
            .unwrap();

        let oldest = repository
            .find_oldest_date_by_user_id("user-1")
            .await
            .unwrap();

        assert_eq!(oldest.as_deref(), Some("2026-04-01"));
    });
}

#[tokio::test]
async fn dashboard_report_for_last_90_days_uses_latest_snapshot_summary() {
    let repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2026-01-15",
            SnapshotValues {
                daily_tss: Some(65),
                ctl: Some(40.0),
                atl: Some(55.0),
                tsb: Some(-15.0),
                ftp_effective_watts: Some(290),
                average_if_28d: Some(0.86),
                average_ef_28d: Some(1.28),
            },
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2026-04-05",
            SnapshotValues {
                daily_tss: Some(82),
                ctl: Some(49.5),
                atl: Some(63.4),
                tsb: Some(-13.9),
                ftp_effective_watts: Some(300),
                average_if_28d: Some(0.89),
                average_ef_28d: Some(1.33),
            },
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2026-04-18",
            SnapshotValues {
                daily_tss: Some(97),
                ctl: Some(62.0),
                atl: Some(37.0),
                tsb: Some(25.0),
                ftp_effective_watts: Some(340),
                average_if_28d: Some(0.92),
                average_ef_28d: Some(1.41),
            },
        ))
        .await
        .unwrap();

    let report = TrainingLoadDashboardReadService::new(repository)
        .build_report(
            "user-1",
            TrainingLoadDashboardRange::Last90Days,
            "2026-04-18",
        )
        .await
        .unwrap();

    assert!(report.has_training_load);
    assert_eq!(report.window_start, "2026-01-19");
    assert_eq!(report.window_end, "2026-04-18");
    assert_eq!(report.points.len(), 2);
    assert_eq!(report.summary.current_ctl, Some(62.0));
    assert_eq!(report.summary.current_atl, Some(37.0));
    assert_eq!(report.summary.current_tsb, Some(25.0));
    assert_eq!(report.summary.ftp_watts, Some(340));
    assert_eq!(report.summary.load_delta_ctl_14d, Some(12.5));
    assert_eq!(report.summary.tsb_zone, TrainingLoadTsbZone::FreshnessPeak);
}

#[tokio::test]
async fn dashboard_report_for_season_starts_on_first_day_of_year() {
    let repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2025-12-31",
            SnapshotValues {
                daily_tss: Some(50),
                ctl: Some(25.0),
                atl: Some(40.0),
                tsb: Some(-15.0),
                ftp_effective_watts: Some(280),
                average_if_28d: None,
                average_ef_28d: None,
            },
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2026-01-01",
            SnapshotValues {
                daily_tss: Some(55),
                ctl: Some(26.0),
                atl: Some(38.0),
                tsb: Some(-12.0),
                ftp_effective_watts: Some(282),
                average_if_28d: None,
                average_ef_28d: None,
            },
        ))
        .await
        .unwrap();

    let report = TrainingLoadDashboardReadService::new(repository)
        .build_report("user-1", TrainingLoadDashboardRange::Season, "2026-04-18")
        .await
        .unwrap();

    assert_eq!(report.window_start, "2026-01-01");
    assert_eq!(report.points.len(), 1);
    assert_eq!(report.points[0].date, "2026-01-01");
}

#[tokio::test]
async fn dashboard_report_for_all_time_uses_oldest_snapshot_date() {
    let repository = InMemoryTrainingLoadDailySnapshotRepository::default();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2025-12-03",
            SnapshotValues {
                daily_tss: Some(0),
                ctl: Some(0.0),
                atl: Some(0.0),
                tsb: Some(0.0),
                ftp_effective_watts: None,
                average_if_28d: None,
                average_ef_28d: None,
            },
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_snapshot_with_values(
            "user-1",
            "2026-04-18",
            SnapshotValues {
                daily_tss: Some(97),
                ctl: Some(29.9),
                atl: Some(53.4),
                tsb: Some(-23.5),
                ftp_effective_watts: Some(340),
                average_if_28d: Some(72.95),
                average_ef_28d: None,
            },
        ))
        .await
        .unwrap();

    let report = TrainingLoadDashboardReadService::new(repository)
        .build_report("user-1", TrainingLoadDashboardRange::AllTime, "2026-04-18")
        .await
        .unwrap();

    assert_eq!(report.window_start, "2025-12-03");
    assert_eq!(report.points.len(), 2);
}

#[tokio::test]
async fn dashboard_report_returns_empty_state_when_user_has_no_snapshots() {
    let report = TrainingLoadDashboardReadService::new(
        InMemoryTrainingLoadDailySnapshotRepository::default(),
    )
    .build_report("user-1", TrainingLoadDashboardRange::AllTime, "2026-04-18")
    .await
    .unwrap();

    assert!(!report.has_training_load);
    assert!(report.points.is_empty());
    assert_eq!(
        report.summary.tsb_zone,
        TrainingLoadTsbZone::OptimalTraining
    );
}

#[test]
fn build_daily_training_load_snapshots_uses_app_ftp_only_on_or_after_app_entry_date() {
    let snapshots = build_daily_training_load_snapshots(
        "user-1",
        &TrainingLoadSnapshotRange {
            oldest: "2026-04-01".to_string(),
            newest: "2026-04-03".to_string(),
        },
        &[
            sample_workout(
                "ride-before",
                "2026-04-01T08:00:00",
                Some(80),
                Some(270),
                Some(300),
                Some(0.8),
                Some(1.2),
            ),
            sample_workout(
                "ride-after",
                "2026-04-03T08:00:00",
                Some(80),
                Some(270),
                Some(300),
                Some(0.81),
                Some(1.21),
            ),
        ],
        &[FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-02".to_string(),
            ftp_watts: 270,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        }],
        "2026-04-02",
        1_700_000_000,
    );

    assert_eq!(snapshots[0].daily_tss, Some(80));
    assert_eq!(snapshots[0].ftp_effective_watts, Some(300));
    assert_eq!(snapshots[0].ftp_source, Some(FtpSource::Provider));
    assert_eq!(snapshots[2].daily_tss, Some(100));
    assert_eq!(snapshots[2].ftp_effective_watts, Some(270));
    assert_eq!(snapshots[2].ftp_source, Some(FtpSource::Settings));
}

#[test]
fn build_daily_training_load_snapshots_keeps_intervals_if_and_ef_averages() {
    let snapshots = build_daily_training_load_snapshots(
        "user-1",
        &TrainingLoadSnapshotRange {
            oldest: "2026-04-01".to_string(),
            newest: "2026-04-03".to_string(),
        },
        &[
            sample_workout(
                "ride-1",
                "2026-04-01T08:00:00",
                Some(70),
                Some(270),
                Some(300),
                Some(0.8),
                Some(1.2),
            ),
            sample_workout(
                "ride-2",
                "2026-04-03T08:00:00",
                Some(90),
                Some(270),
                Some(300),
                Some(0.9),
                Some(1.4),
            ),
        ],
        &[FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-01".to_string(),
            ftp_watts: 270,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        }],
        "2026-04-01",
        1_700_000_000,
    );

    let last = snapshots.last().unwrap();
    assert_eq!(last.average_if_28d, Some(0.85));
    assert_eq!(last.average_ef_28d, Some(1.3));
}

#[test]
fn build_daily_training_load_snapshots_falls_back_to_provider_ftp_after_ftp_is_cleared() {
    let snapshots = build_daily_training_load_snapshots(
        "user-1",
        &TrainingLoadSnapshotRange {
            oldest: "2026-04-01".to_string(),
            newest: "2026-04-03".to_string(),
        },
        &[
            sample_workout(
                "ride-before-clear",
                "2026-04-01T08:00:00",
                Some(70),
                Some(270),
                Some(300),
                Some(0.8),
                Some(1.2),
            ),
            sample_workout(
                "ride-after-clear",
                "2026-04-03T08:00:00",
                Some(80),
                Some(260),
                Some(300),
                Some(0.81),
                Some(1.21),
            ),
        ],
        &[
            FtpHistoryEntry {
                user_id: "user-1".to_string(),
                effective_from_date: "2026-04-01".to_string(),
                ftp_watts: 270,
                source: FtpSource::Settings,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            },
            FtpHistoryEntry {
                user_id: "user-1".to_string(),
                effective_from_date: "2026-04-02".to_string(),
                ftp_watts: 0,
                source: FtpSource::Settings,
                created_at_epoch_seconds: 2,
                updated_at_epoch_seconds: 2,
            },
        ],
        "2026-04-01",
        1_700_000_000,
    );

    assert_eq!(snapshots[0].ftp_effective_watts, Some(270));
    assert_eq!(snapshots[0].ftp_source, Some(FtpSource::Settings));
    assert_eq!(snapshots[2].ftp_effective_watts, Some(300));
    assert_eq!(snapshots[2].ftp_source, Some(FtpSource::Provider));
    assert_eq!(snapshots[2].daily_tss, Some(80));
}

#[tokio::test]
async fn recompute_from_rebuilds_snapshots_from_warmup_start() {
    #[derive(Clone, Default)]
    struct InMemoryCompletedWorkoutRepository {
        workouts: std::sync::Arc<std::sync::Mutex<Vec<CompletedWorkout>>>,
    }

    impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
        fn find_by_user_id_and_completed_workout_id(
            &self,
            user_id: &str,
            completed_workout_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            let completed_workout_id = completed_workout_id.to_string();
            Box::pin(async move {
                Ok(workouts.lock().unwrap().iter().find_map(|workout| {
                    (workout.user_id == user_id
                        && workout.completed_workout_id == completed_workout_id)
                        .then(|| workout.clone())
                }))
            })
        }

        fn find_by_user_id_and_source_activity_id(
            &self,
            user_id: &str,
            source_activity_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            let source_activity_id = source_activity_id.to_string();
            Box::pin(async move {
                Ok(workouts.lock().unwrap().iter().find_map(|workout| {
                    (workout.user_id == user_id
                        && workout.source_activity_id.as_deref()
                            == Some(source_activity_id.as_str()))
                    .then(|| workout.clone())
                }))
            })
        }

        fn find_latest_by_user_id(
            &self,
            user_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                let mut values = workouts
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .cloned()
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| right.start_date_local.cmp(&left.start_date_local));
                Ok(values.into_iter().next())
            })
        }

        fn list_by_user_id(
            &self,
            user_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                let mut values = workouts
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .cloned()
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| left.start_date_local.cmp(&right.start_date_local));
                Ok(values)
            })
        }

        fn list_by_user_id_and_date_range(
            &self,
            user_id: &str,
            oldest: &str,
            newest: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            let oldest = oldest.to_string();
            let newest = newest.to_string();
            Box::pin(async move {
                let mut values = workouts
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .filter(|workout| {
                        let date = workout.start_date_local.get(..10).unwrap_or_default();
                        date >= oldest.as_str() && date <= newest.as_str()
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| left.start_date_local.cmp(&right.start_date_local));
                Ok(values)
            })
        }

        fn upsert(
            &self,
            workout: CompletedWorkout,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            let workouts = self.workouts.clone();
            Box::pin(async move {
                let mut workouts = workouts.lock().unwrap();
                workouts.retain(|existing| {
                    !(existing.user_id == workout.user_id
                        && existing.completed_workout_id == workout.completed_workout_id)
                });
                workouts.push(workout.clone());
                Ok(workout)
            })
        }
    }

    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    completed_workouts
        .upsert(sample_workout(
            "ride-1",
            "2026-04-03T08:00:00",
            Some(80),
            Some(270),
            Some(300),
            Some(0.8),
            Some(1.2),
        ))
        .await
        .unwrap();
    let ftp_history = InMemoryFtpHistoryRepository::default();
    ftp_history
        .upsert(FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2026-04-02".to_string(),
            ftp_watts: 270,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        })
        .await
        .unwrap();
    let snapshots = InMemoryTrainingLoadDailySnapshotRepository::default();
    let settings_repository = StaticSettingsRepository {
        settings: UserSettings {
            user_id: "user-1".to_string(),
            ai_agents: AiAgentsConfig::default(),
            intervals: IntervalsConfig::default(),
            options: AnalysisOptions::default(),
            availability: AvailabilitySettings::default(),
            cycling: CyclingSettings {
                ftp_watts: Some(270),
                ..CyclingSettings::default()
            },
            created_at_epoch_seconds: 1_743_465_600,
            updated_at_epoch_seconds: 1_743_465_600,
        },
    };
    let service = TrainingLoadRecomputeService::new(
        completed_workouts,
        ftp_history,
        snapshots.clone(),
        settings_repository,
    )
    .with_warmup_days(2);

    service
        .recompute_from("user-1", "2026-04-03", 1_775_174_400)
        .await
        .unwrap();

    let stored = snapshots.stored();
    assert_eq!(
        stored.first().map(|snapshot| snapshot.date.as_str()),
        Some("2026-04-01")
    );
    assert_eq!(
        stored.last().map(|snapshot| snapshot.date.as_str()),
        Some("2026-04-03")
    );
    assert_eq!(
        stored
            .iter()
            .find(|snapshot| snapshot.date == "2026-04-03")
            .and_then(|snapshot| snapshot.daily_tss),
        Some(100)
    );
}

#[tokio::test]
async fn recompute_from_keeps_existing_snapshots_when_upsert_fails() {
    #[derive(Clone, Default)]
    struct DeleteTrackingSnapshotRepository {
        stored: std::sync::Arc<std::sync::Mutex<Vec<TrainingLoadDailySnapshot>>>,
        delete_calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    }

    impl DeleteTrackingSnapshotRepository {
        fn stored(&self) -> Vec<TrainingLoadDailySnapshot> {
            self.stored.lock().unwrap().clone()
        }

        fn delete_calls(&self) -> Vec<(String, String)> {
            self.delete_calls.lock().unwrap().clone()
        }
    }

    impl crate::domain::training_load::TrainingLoadDailySnapshotRepository
        for DeleteTrackingSnapshotRepository
    {
        fn list_by_user_id_and_range(
            &self,
            _user_id: &str,
            _range: &TrainingLoadSnapshotRange,
        ) -> crate::domain::training_load::BoxFuture<
            Result<Vec<TrainingLoadDailySnapshot>, TrainingLoadError>,
        > {
            let stored = self.stored.clone();
            Box::pin(async move { Ok(stored.lock().unwrap().clone()) })
        }

        fn upsert(
            &self,
            _snapshot: TrainingLoadDailySnapshot,
        ) -> crate::domain::training_load::BoxFuture<
            Result<TrainingLoadDailySnapshot, TrainingLoadError>,
        > {
            Box::pin(async {
                Err(TrainingLoadError::Repository(
                    "snapshot upsert failed".to_string(),
                ))
            })
        }

        fn delete_by_user_id_from_date(
            &self,
            user_id: &str,
            from_date: &str,
        ) -> crate::domain::training_load::BoxFuture<Result<(), TrainingLoadError>> {
            let delete_calls = self.delete_calls.clone();
            let user_id = user_id.to_string();
            let from_date = from_date.to_string();
            Box::pin(async move {
                delete_calls.lock().unwrap().push((user_id, from_date));
                Ok(())
            })
        }
    }

    let snapshots = DeleteTrackingSnapshotRepository::default();
    snapshots
        .stored
        .lock()
        .unwrap()
        .push(TrainingLoadDailySnapshot {
            user_id: "user-1".to_string(),
            date: "2026-04-01".to_string(),
            daily_tss: Some(42),
            rolling_tss_7d: Some(6.0),
            rolling_tss_28d: Some(1.5),
            ctl: Some(2.0),
            atl: Some(7.0),
            tsb: Some(-5.0),
            average_if_28d: Some(0.8),
            average_ef_28d: Some(1.2),
            ftp_effective_watts: Some(270),
            ftp_source: Some(FtpSource::Settings),
            recomputed_at_epoch_seconds: 1,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        });

    let service = TrainingLoadRecomputeService::new(
        EmptyCompletedWorkoutRepository,
        InMemoryFtpHistoryRepository::default(),
        snapshots.clone(),
        StaticSettingsRepository {
            settings: UserSettings::new_defaults("user-1".to_string(), 1_699_315_200),
        },
    )
    .with_warmup_days(2);

    let result = service
        .recompute_from("user-1", "2026-04-03", 1_775_174_400)
        .await;

    assert!(result.is_err());
    assert!(snapshots.delete_calls().is_empty());
    assert_eq!(snapshots.stored().len(), 1);
    assert_eq!(snapshots.stored()[0].date, "2026-04-01");
}

#[tokio::test]
async fn recompute_from_rejects_invalid_oldest_date() {
    let service = TrainingLoadRecomputeService::new(
        EmptyCompletedWorkoutRepository,
        InMemoryFtpHistoryRepository::default(),
        InMemoryTrainingLoadDailySnapshotRepository::default(),
        StaticSettingsRepository {
            settings: UserSettings::new_defaults("user-1".to_string(), 1_699_315_200),
        },
    );

    let result = service
        .recompute_from("user-1", "not-a-date", 1_775_174_400)
        .await;

    assert!(matches!(
        result,
        Err(TrainingLoadError::Repository(message))
            if message.contains("invalid oldest_date 'not-a-date'")
    ));
}

fn sample_workout(
    completed_workout_id: &str,
    start_date_local: &str,
    training_stress_score: Option<i32>,
    normalized_power_watts: Option<i32>,
    ftp_watts: Option<i32>,
    intensity_factor: Option<f64>,
    efficiency_factor: Option<f64>,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        "user-1".to_string(),
        start_date_local.to_string(),
        None,
        None,
        Some(completed_workout_id.to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        Some(3600),
        None,
        CompletedWorkoutMetrics {
            training_stress_score,
            normalized_power_watts,
            intensity_factor,
            efficiency_factor,
            variability_index: None,
            average_power_watts: None,
            ftp_watts,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::<CompletedWorkoutInterval>::new(),
            interval_groups: Vec::<CompletedWorkoutIntervalGroup>::new(),
            streams: Vec::<CompletedWorkoutStream>::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::<CompletedWorkoutZoneTime>::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

struct SnapshotValues {
    daily_tss: Option<i32>,
    ctl: Option<f64>,
    atl: Option<f64>,
    tsb: Option<f64>,
    ftp_effective_watts: Option<i32>,
    average_if_28d: Option<f64>,
    average_ef_28d: Option<f64>,
}

fn sample_snapshot_with_values(
    user_id: &str,
    date: &str,
    values: SnapshotValues,
) -> TrainingLoadDailySnapshot {
    TrainingLoadDailySnapshot {
        user_id: user_id.to_string(),
        date: date.to_string(),
        daily_tss: values.daily_tss,
        rolling_tss_7d: None,
        rolling_tss_28d: None,
        ctl: values.ctl,
        atl: values.atl,
        tsb: values.tsb,
        average_if_28d: values.average_if_28d,
        average_ef_28d: values.average_ef_28d,
        ftp_effective_watts: values.ftp_effective_watts,
        ftp_source: values.ftp_effective_watts.map(|_| FtpSource::Settings),
        recomputed_at_epoch_seconds: 100,
        created_at_epoch_seconds: 100,
        updated_at_epoch_seconds: 100,
    }
}
