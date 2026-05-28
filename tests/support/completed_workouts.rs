use aiwattcoach::domain::completed_workouts::{
    CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutRepository,
};
use std::sync::{Arc, Mutex};

pub(crate) fn completed_workout(
    completed_workout_id: &str,
    source_activity_id: Option<&str>,
    planned_workout_id: Option<&str>,
    external_id: Option<&str>,
    start_date_local: &str,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        "user-1".to_string(),
        start_date_local.to_string(),
        source_activity_id.map(ToString::to_string),
        planned_workout_id.map(ToString::to_string),
        Some("Aerobic Endurance".to_string()),
        None,
        Some("Ride".to_string()),
        external_id.map(ToString::to_string),
        false,
        Some(5283),
        Some(40000.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(61),
            normalized_power_watts: Some(223),
            intensity_factor: Some(0.66),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(207),
            ftp_watts: Some(335),
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
pub(crate) struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl InMemoryCompletedWorkoutRepository {
    pub(crate) fn with_workouts(workouts: Vec<CompletedWorkout>) -> Self {
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
                .filter(|workout| {
                    workout.user_id == user_id
                        && workout.start_date_local.as_str() >= oldest.as_str()
                        && workout.start_date_local.as_str() <= newest.as_str()
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
            if let Some(existing) = stored.iter_mut().find(|existing| {
                existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id
            }) {
                *existing = workout.clone();
            } else {
                stored.push(workout.clone());
            }
            Ok(workout)
        })
    }
}
