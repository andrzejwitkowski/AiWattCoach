use std::sync::{Arc, Mutex};

use crate::domain::planned_workouts::{
    BoxFuture, PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository,
};

#[derive(Clone, Default)]
pub struct RecordingPlannedWorkoutRepository {
    stored: Arc<Mutex<Vec<PlannedWorkout>>>,
    upserted: Arc<Mutex<Vec<PlannedWorkout>>>,
    operation_log: Arc<Mutex<Vec<String>>>,
    shared_log: Option<Arc<Mutex<Vec<String>>>>,
}

impl RecordingPlannedWorkoutRepository {
    pub fn with_workouts(workouts: Vec<PlannedWorkout>) -> Self {
        Self {
            stored: Arc::new(Mutex::new(workouts)),
            upserted: Arc::new(Mutex::new(Vec::new())),
            operation_log: Arc::new(Mutex::new(Vec::new())),
            shared_log: None,
        }
    }

    pub fn with_workouts_and_shared_log(
        workouts: Vec<PlannedWorkout>,
        shared_log: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            stored: Arc::new(Mutex::new(workouts)),
            upserted: Arc::new(Mutex::new(Vec::new())),
            operation_log: Arc::new(Mutex::new(Vec::new())),
            shared_log: Some(shared_log),
        }
    }

    pub fn stored(&self) -> Vec<PlannedWorkout> {
        self.stored
            .lock()
            .expect("planned workouts mutex poisoned")
            .clone()
    }

    pub fn upserted(&self) -> Vec<PlannedWorkout> {
        self.upserted
            .lock()
            .expect("planned workouts mutex poisoned")
            .clone()
    }

    pub fn operation_log(&self) -> Vec<String> {
        self.operation_log
            .lock()
            .expect("planned workouts mutex poisoned")
            .clone()
    }
}

impl PlannedWorkoutRepository for RecordingPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("planned workouts mutex poisoned")
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
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("planned workouts mutex poisoned")
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, PlannedWorkoutError>> {
        let stored = self.stored.clone();
        let upserted = self.upserted.clone();
        let operation_log = self.operation_log.clone();
        let shared_log = self.shared_log.clone();
        Box::pin(async move {
            operation_log
                .lock()
                .expect("planned workouts mutex poisoned")
                .push("planned_workouts.upsert".to_string());
            if let Some(shared_log) = shared_log {
                shared_log
                    .lock()
                    .expect("shared log mutex poisoned")
                    .push("planned_workouts.upsert".to_string());
            }
            upserted
                .lock()
                .expect("planned workouts mutex poisoned")
                .push(workout.clone());
            let mut stored = stored.lock().expect("planned workouts mutex poisoned");
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.planned_workout_id == workout.planned_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}
