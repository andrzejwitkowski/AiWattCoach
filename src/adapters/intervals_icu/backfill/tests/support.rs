use std::sync::{Arc, Mutex};

use crate::domain::{
    completed_workouts::{
        BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutDetails,
        CompletedWorkoutError, CompletedWorkoutMetrics, CompletedWorkoutRepository,
    },
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalImportCommand, ExternalImportError,
        ExternalImportOutcome, ExternalImportUseCases,
    },
    identity::Clock,
    intervals::{
        Activity, ActivityDetails, ActivityMetrics, BoxFuture, DateRange, Event,
        IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
    },
    training_load::{
        BoxFuture as TrainingLoadBoxFuture, TrainingLoadError, TrainingLoadRecomputeUseCases,
    },
};

#[derive(Clone, Copy)]
pub(super) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_775_174_400
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingRecomputeService {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingRecomputeService {
    pub(super) fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TrainingLoadRecomputeUseCases for RecordingRecomputeService {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> TrainingLoadBoxFuture<Result<(), TrainingLoadError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest_date, now_epoch_seconds));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct TestCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let stored = stored
                .lock()
                .expect("completed workout repo mutex poisoned");
            Ok(stored.iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            let stored = stored
                .lock()
                .expect("completed workout repo mutex poisoned");
            Ok(stored.iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
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
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
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
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
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
pub(super) struct TestSettings;

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
pub(super) struct TestApi {
    pub(super) activity: Activity,
    pub(super) lookups: Arc<Mutex<Vec<String>>>,
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
pub(super) struct RecordingImports {
    commands: Arc<Mutex<Vec<ExternalImportCommand>>>,
}

impl RecordingImports {
    pub(super) fn commands(&self) -> Vec<ExternalImportCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl ExternalImportUseCases for RecordingImports {
    fn import(
        &self,
        command: ExternalImportCommand,
    ) -> crate::domain::external_sync::BoxFuture<Result<ExternalImportOutcome, ExternalImportError>>
    {
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

pub(super) fn sample_sparse_workout() -> CompletedWorkout {
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

pub(super) fn sample_detailed_activity() -> Activity {
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

pub(super) fn sample_complete_workout() -> CompletedWorkout {
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

pub(super) fn sample_unbackfillable_activity() -> Activity {
    let mut activity = sample_detailed_activity();
    activity.details.streams.clear();
    activity.details.intervals.clear();
    activity.details.interval_groups.clear();
    activity.details.interval_summary.clear();
    activity.details.skyline_chart.clear();
    activity.details.power_zone_times.clear();
    activity.details.heart_rate_zone_times.clear();
    activity.details.pace_zone_times.clear();
    activity.details.gap_zone_times.clear();
    activity
}

pub(super) fn sample_metrics_unbackfillable_activity() -> Activity {
    let mut activity = sample_detailed_activity();
    activity.metrics.training_stress_score = None;
    activity.metrics.ftp_watts = None;
    activity.metrics.normalized_power_watts = None;
    activity
}

pub(super) fn sample_workout_missing_tss() -> CompletedWorkout {
    let mut workout = sample_sparse_workout();
    workout.metrics.training_stress_score = None;
    workout
}
