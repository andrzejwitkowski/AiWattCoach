use aiwattcoach::{
    adapters::{
        workout_summary_completed_target::CompletedWorkoutTargetAdapter,
        workout_summary_latest_activity::LatestCompletedActivityAdapter,
    },
    domain::{
        completed_workouts::{
            CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
            CompletedWorkoutRepository,
        },
        workout_summary::{CompletedWorkoutTargetUseCases, LatestCompletedActivityUseCases},
    },
};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn completed_workout_target_adapter_accepts_legacy_completed_workout_ids() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:legacy-41",
        "2026-03-22T08:00:00",
    )]);
    let adapter = CompletedWorkoutTargetAdapter::new(repository);

    let is_target = adapter
        .is_completed_workout_target("user-1", "legacy-41")
        .await
        .expect("target lookup should succeed");

    assert!(is_target);
}

#[tokio::test]
async fn completed_workout_target_adapter_accepts_canonical_completed_workout_ids() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:legacy-41",
        "2026-03-22T08:00:00",
    )]);
    let adapter = CompletedWorkoutTargetAdapter::new(repository);

    let is_target = adapter
        .is_completed_workout_target("user-1", "intervals-activity:legacy-41")
        .await
        .expect("target lookup should succeed");

    assert!(is_target);
}

#[tokio::test]
async fn latest_completed_activity_adapter_falls_back_to_legacy_completed_workout_id() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:latest-77",
        "2026-03-22T08:00:00",
    )]);
    let adapter = LatestCompletedActivityAdapter::new(repository);

    let latest_activity_id = adapter
        .latest_completed_activity_id("user-1")
        .await
        .expect("latest lookup should succeed");

    assert_eq!(latest_activity_id.as_deref(), Some("latest-77"));
}

fn completed_workout_repository(
    workouts: Vec<CompletedWorkout>,
) -> impl CompletedWorkoutRepository {
    InMemoryCompletedWorkoutRepository::with_workouts(workouts)
}

fn legacy_completed_workout(
    completed_workout_id: &str,
    start_date_local: &str,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        "user-1".to_string(),
        start_date_local.to_string(),
        None,
        None,
        Some("Legacy Ride".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        Some(3600),
        Some(40000.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(72),
            normalized_power_watts: Some(238),
            intensity_factor: Some(0.84),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(228),
            ftp_watts: Some(283),
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

#[derive(Clone, Default)]
struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl InMemoryCompletedWorkoutRepository {
    fn with_workouts(workouts: Vec<CompletedWorkout>) -> Self {
        Self {
            stored: Arc::new(Mutex::new(workouts)),
        }
    }
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Option<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Option<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Option<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Vec<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
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
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Vec<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
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
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, aiwattcoach::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}
