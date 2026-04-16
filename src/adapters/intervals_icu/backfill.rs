use crate::{
    adapters::intervals_icu::import_mapping::map_activity_to_import_command,
    domain::{
        completed_workouts::{
            BackfillCompletedWorkoutDetailsResult, CompletedWorkout, CompletedWorkoutAdminUseCases,
            CompletedWorkoutError, CompletedWorkoutRepository,
        },
        external_sync::ExternalImportUseCases,
        intervals::{IntervalsApiPort, IntervalsSettingsPort},
    },
};

#[derive(Clone)]
pub struct IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
{
    repository: Repo,
    settings: Settings,
    api: Api,
    imports: Imports,
}

impl<Repo, Settings, Api, Imports>
    IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
{
    pub fn new(repository: Repo, settings: Settings, api: Api, imports: Imports) -> Self {
        Self {
            repository,
            settings,
            api,
            imports,
        }
    }
}

impl<Repo, Settings, Api, Imports> CompletedWorkoutAdminUseCases
    for IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
{
    fn backfill_missing_details(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<BackfillCompletedWorkoutDetailsResult, CompletedWorkoutError>,
    > {
        let repository = self.repository.clone();
        let settings = self.settings.clone();
        let api = self.api.clone();
        let imports = self.imports.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();

        Box::pin(async move {
            let workouts = repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await?;
            let scanned = workouts.len();

            let candidates = workouts
                .into_iter()
                .filter(needs_detail_backfill)
                .collect::<Vec<_>>();

            if candidates.is_empty() {
                return Ok(BackfillCompletedWorkoutDetailsResult {
                    scanned,
                    enriched: 0,
                    skipped: scanned,
                    failed: 0,
                });
            }

            let credentials = settings
                .get_credentials(&user_id)
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;

            let mut enriched = 0;
            let mut skipped = 0;
            let mut failed = 0;

            for workout in &candidates {
                let Some(source_activity_id) = workout.source_activity_id.as_deref() else {
                    skipped += 1;
                    continue;
                };

                let detailed_activity =
                    match api.get_activity(&credentials, source_activity_id).await {
                        Ok(activity) => activity,
                        Err(_) => {
                            failed += 1;
                            continue;
                        }
                    };

                if !has_backfillable_activity_details(&detailed_activity) {
                    skipped += 1;
                    continue;
                }

                match imports
                    .import(map_activity_to_import_command(&user_id, &detailed_activity))
                    .await
                {
                    Ok(_) => enriched += 1,
                    Err(_) => failed += 1,
                }
            }

            Ok(BackfillCompletedWorkoutDetailsResult {
                scanned,
                enriched,
                skipped: skipped + scanned.saturating_sub(candidates.len()),
                failed,
            })
        })
    }
}

fn needs_detail_backfill(workout: &CompletedWorkout) -> bool {
    workout.source_activity_id.is_some()
        && workout.details_unavailable_reason.is_none()
        && !has_any_backfillable_details(&workout.details)
}

fn has_backfillable_activity_details(activity: &crate::domain::intervals::Activity) -> bool {
    !activity.details.streams.is_empty()
        || !activity.details.intervals.is_empty()
        || !activity.details.interval_groups.is_empty()
        || !activity.details.interval_summary.is_empty()
        || !activity.details.skyline_chart.is_empty()
        || !activity.details.power_zone_times.is_empty()
        || !activity.details.heart_rate_zone_times.is_empty()
        || !activity.details.pace_zone_times.is_empty()
        || !activity.details.gap_zone_times.is_empty()
}

fn has_any_backfillable_details(
    details: &crate::domain::completed_workouts::CompletedWorkoutDetails,
) -> bool {
    !details.streams.is_empty()
        || !details.intervals.is_empty()
        || !details.interval_groups.is_empty()
        || !details.interval_summary.is_empty()
        || !details.skyline_chart.is_empty()
        || !details.power_zone_times.is_empty()
        || !details.heart_rate_zone_times.is_empty()
        || !details.pace_zone_times.is_empty()
        || !details.gap_zone_times.is_empty()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        completed_workouts::{
            BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout,
            CompletedWorkoutAdminUseCases, CompletedWorkoutDetails, CompletedWorkoutError,
            CompletedWorkoutMetrics, CompletedWorkoutRepository,
        },
        external_sync::{
            CanonicalEntityKind, CanonicalEntityRef, ExternalImportCommand, ExternalImportError,
            ExternalImportOutcome, ExternalImportUseCases,
        },
        intervals::{
            Activity, ActivityDetails, ActivityMetrics, BoxFuture, DateRange, Event,
            IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
        },
    };

    use super::IntervalsCompletedWorkoutBackfillService;

    #[derive(Clone, Default)]
    struct TestCompletedWorkoutRepository {
        stored: Arc<Mutex<Vec<CompletedWorkout>>>,
    }

    impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
        fn find_by_user_id_and_completed_workout_id(
            &self,
            user_id: &str,
            completed_workout_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let completed_workout_id = completed_workout_id.to_string();
            Box::pin(async move {
                let stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                Ok(stored.iter().find_map(|workout| {
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
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let source_activity_id = source_activity_id.to_string();
            Box::pin(async move {
                let stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                Ok(stored.iter().find_map(|workout| {
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
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                let stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                let mut workouts = stored
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .cloned()
                    .collect::<Vec<_>>();
                workouts.sort_by(|left, right| right.start_date_local.cmp(&left.start_date_local));
                Ok(workouts.into_iter().next())
            })
        }

        fn list_by_user_id(
            &self,
            user_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                let stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                Ok(stored
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .cloned()
                    .collect())
            })
        }

        fn list_by_user_id_and_date_range(
            &self,
            user_id: &str,
            oldest: &str,
            newest: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let oldest = oldest.to_string();
            let newest = newest.to_string();
            Box::pin(async move {
                let stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                Ok(stored
                    .iter()
                    .filter(|workout| workout.user_id == user_id)
                    .filter(|workout| {
                        let date = workout.start_date_local.get(..10).unwrap_or_default();
                        date >= oldest.as_str() && date <= newest.as_str()
                    })
                    .cloned()
                    .collect())
            })
        }

        fn upsert(
            &self,
            workout: CompletedWorkout,
        ) -> CompletedWorkoutBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
            let stored = self.stored.clone();
            Box::pin(async move {
                let mut stored = stored
                    .lock()
                    .expect("completed workout repo mutex poisoned");
                stored.retain(|existing| {
                    !(existing.user_id == workout.user_id
                        && existing.completed_workout_id == workout.completed_workout_id)
                });
                stored.push(workout.clone());
                Ok(workout)
            })
        }
    }

    #[derive(Clone, Default)]
    struct TestSettings;

    impl IntervalsSettingsPort for TestSettings {
        fn get_credentials(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
            Box::pin(async {
                Ok(IntervalsCredentials {
                    api_key: "key".to_string(),
                    athlete_id: "athlete".to_string(),
                })
            })
        }
    }

    #[derive(Clone)]
    struct TestApi {
        activity: Activity,
        lookups: Arc<Mutex<Vec<String>>>,
    }

    impl crate::domain::intervals::IntervalsApiPort for TestApi {
        fn list_events(
            &self,
            _credentials: &IntervalsCredentials,
            _range: &DateRange,
        ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn get_event(
            &self,
            _credentials: &IntervalsCredentials,
            _event_id: i64,
        ) -> BoxFuture<Result<Event, IntervalsError>> {
            Box::pin(async { unreachable!() })
        }

        fn create_event(
            &self,
            _credentials: &IntervalsCredentials,
            _event: crate::domain::intervals::CreateEvent,
        ) -> BoxFuture<Result<Event, IntervalsError>> {
            Box::pin(async { unreachable!() })
        }

        fn update_event(
            &self,
            _credentials: &IntervalsCredentials,
            _event_id: i64,
            _event: crate::domain::intervals::UpdateEvent,
        ) -> BoxFuture<Result<Event, IntervalsError>> {
            Box::pin(async { unreachable!() })
        }

        fn delete_event(
            &self,
            _credentials: &IntervalsCredentials,
            _event_id: i64,
        ) -> BoxFuture<Result<(), IntervalsError>> {
            Box::pin(async { unreachable!() })
        }

        fn download_fit(
            &self,
            _credentials: &IntervalsCredentials,
            _event_id: i64,
        ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
            Box::pin(async { unreachable!() })
        }

        fn get_activity(
            &self,
            _credentials: &IntervalsCredentials,
            activity_id: &str,
        ) -> BoxFuture<Result<Activity, IntervalsError>> {
            let lookups = self.lookups.clone();
            let activity = self.activity.clone();
            let activity_id = activity_id.to_string();
            Box::pin(async move {
                lookups.lock().unwrap().push(activity_id);
                Ok(activity)
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingImports {
        commands: Arc<Mutex<Vec<ExternalImportCommand>>>,
    }

    impl RecordingImports {
        fn commands(&self) -> Vec<ExternalImportCommand> {
            self.commands.lock().unwrap().clone()
        }
    }

    impl ExternalImportUseCases for RecordingImports {
        fn import(
            &self,
            command: ExternalImportCommand,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<ExternalImportOutcome, ExternalImportError>,
        > {
            let commands = self.commands.clone();
            Box::pin(async move {
                commands.lock().unwrap().push(command.clone());
                Ok(ExternalImportOutcome {
                    canonical_entity: CanonicalEntityRef::new(
                        CanonicalEntityKind::CompletedWorkout,
                        "intervals-activity:i1".to_string(),
                    ),
                    provider: crate::domain::external_sync::ExternalProvider::Intervals,
                    external_id: "i1".to_string(),
                })
            })
        }
    }

    #[tokio::test]
    async fn backfill_missing_details_imports_enriched_intervals_activity() {
        let repository = TestCompletedWorkoutRepository::default();
        repository.upsert(sample_sparse_workout()).await.unwrap();
        let lookups = Arc::new(Mutex::new(Vec::new()));
        let api = TestApi {
            activity: sample_detailed_activity(),
            lookups: lookups.clone(),
        };
        let imports = RecordingImports::default();
        let service = IntervalsCompletedWorkoutBackfillService::new(
            repository,
            TestSettings,
            api,
            imports.clone(),
        );

        let result = service
            .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
            .await
            .unwrap();

        assert_eq!(result.scanned, 1);
        assert_eq!(result.enriched, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(lookups.lock().unwrap().as_slice(), &["i1".to_string()]);

        let commands = imports.commands();
        let ExternalImportCommand::UpsertCompletedWorkout(import) = &commands[0] else {
            panic!("expected completed workout import");
        };
        assert!(!import.workout.details.streams.is_empty());
    }

    #[tokio::test]
    async fn backfill_missing_details_reports_full_scanned_range() {
        let repository = TestCompletedWorkoutRepository::default();
        repository.upsert(sample_sparse_workout()).await.unwrap();
        repository.upsert(sample_complete_workout()).await.unwrap();
        let imports = RecordingImports::default();
        let service = IntervalsCompletedWorkoutBackfillService::new(
            repository,
            TestSettings,
            TestApi {
                activity: sample_detailed_activity(),
                lookups: Arc::new(Mutex::new(Vec::new())),
            },
            imports,
        );

        let result = service
            .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
            .await
            .unwrap();

        assert_eq!(result.scanned, 2);
        assert_eq!(result.enriched, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn backfill_missing_details_skips_summary_only_workout() {
        let repository = TestCompletedWorkoutRepository::default();
        repository
            .upsert(sample_summary_only_workout())
            .await
            .unwrap();
        let imports = RecordingImports::default();
        let service = IntervalsCompletedWorkoutBackfillService::new(
            repository,
            TestSettings,
            TestApi {
                activity: sample_summary_only_activity(),
                lookups: Arc::new(Mutex::new(Vec::new())),
            },
            imports.clone(),
        );

        let result = service
            .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
            .await
            .unwrap();

        assert_eq!(result.scanned, 1);
        assert_eq!(result.enriched, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);
        assert!(imports.commands().is_empty());
    }

    fn sample_sparse_workout() -> CompletedWorkout {
        CompletedWorkout::new(
            "intervals-activity:i1".to_string(),
            "user-1".to_string(),
            "2026-04-16T15:28:24".to_string(),
            Some("i1".to_string()),
            None,
            Some("Ride".to_string()),
            None,
            Some("Ride".to_string()),
            Some("external-1".to_string()),
            false,
            Some(3600),
            Some(35000.0),
            CompletedWorkoutMetrics {
                training_stress_score: Some(80),
                normalized_power_watts: Some(250),
                intensity_factor: Some(0.8),
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: Some(220),
                ftp_watts: Some(300),
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            CompletedWorkoutDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: Vec::new(),
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            None,
        )
    }

    fn sample_detailed_activity() -> Activity {
        Activity {
            id: "i1".to_string(),
            athlete_id: Some("athlete".to_string()),
            start_date_local: "2026-04-16T15:28:24".to_string(),
            start_date: None,
            name: Some("Ride".to_string()),
            description: None,
            activity_type: Some("Ride".to_string()),
            source: None,
            external_id: Some("external-1".to_string()),
            device_name: None,
            distance_meters: Some(35000.0),
            moving_time_seconds: Some(3600),
            elapsed_time_seconds: Some(3600),
            total_elevation_gain_meters: None,
            total_elevation_loss_meters: None,
            average_speed_mps: None,
            max_speed_mps: None,
            average_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            average_cadence_rpm: Some(88.0),
            trainer: false,
            commute: false,
            race: false,
            has_heart_rate: false,
            stream_types: vec!["watts".to_string(), "cadence".to_string()],
            tags: Vec::new(),
            metrics: ActivityMetrics {
                training_stress_score: Some(80),
                normalized_power_watts: Some(250),
                intensity_factor: Some(0.8),
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: Some(220),
                ftp_watts: Some(300),
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            details: ActivityDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: vec![crate::domain::intervals::ActivityStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    data: Some(serde_json::json!([180, 240, 310, 330])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            details_unavailable_reason: None,
        }
    }

    fn sample_complete_workout() -> CompletedWorkout {
        let mut workout = sample_sparse_workout();
        workout.completed_workout_id = "intervals-activity:i2".to_string();
        workout.source_activity_id = Some("i2".to_string());
        workout.details.intervals = vec![
            crate::domain::completed_workouts::CompletedWorkoutInterval {
                id: Some(1),
                label: Some("Block 1".to_string()),
                interval_type: Some("work".to_string()),
                group_id: Some("g1".to_string()),
                start_index: Some(0),
                end_index: Some(1),
                start_time_seconds: Some(0),
                end_time_seconds: Some(60),
                moving_time_seconds: Some(60),
                elapsed_time_seconds: Some(60),
                distance_meters: Some(500.0),
                average_power_watts: Some(250),
                normalized_power_watts: Some(255),
                training_stress_score: Some(5.0),
                average_heart_rate_bpm: None,
                average_cadence_rpm: Some(90.0),
                average_speed_mps: None,
                average_stride_meters: None,
                zone: Some(3),
            },
        ];
        workout.details.streams = vec![crate::domain::completed_workouts::CompletedWorkoutStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            primary_series: Some(
                crate::domain::completed_workouts::CompletedWorkoutSeries::Integers(vec![180, 240]),
            ),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        }];
        workout
    }

    fn sample_summary_only_workout() -> CompletedWorkout {
        let mut workout = sample_sparse_workout();
        workout.completed_workout_id = "intervals-activity:i3".to_string();
        workout.source_activity_id = Some("i3".to_string());
        workout.details.interval_summary = vec!["tempo summary".to_string()];
        workout
    }

    fn sample_summary_only_activity() -> Activity {
        let mut activity = sample_detailed_activity();
        activity.id = "i3".to_string();
        activity.details.streams.clear();
        activity.details.intervals.clear();
        activity.details.interval_groups.clear();
        activity.details.interval_summary = vec!["tempo summary".to_string()];
        activity.details.power_zone_times = vec![crate::domain::intervals::ActivityZoneTime {
            zone_id: "z3".to_string(),
            seconds: 1200,
        }];
        activity
    }
}
