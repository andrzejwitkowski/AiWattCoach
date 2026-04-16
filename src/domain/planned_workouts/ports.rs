use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{PlannedWorkout, PlannedWorkoutError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedWorkoutRepository: Clone + Send + Sync + 'static {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>;

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, PlannedWorkoutError>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopPlannedWorkoutRepository {
    stored: Arc<Mutex<Vec<PlannedWorkout>>>,
}

#[cfg(test)]
impl PlannedWorkoutRepository for NoopPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("planned workout repo mutex poisoned");
            let mut workouts = stored
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.planned_workout_id.cmp(&right.planned_workout_id))
            });
            Ok(workouts)
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
            let stored = stored.lock().expect("planned workout repo mutex poisoned");
            let mut workouts = stored
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.planned_workout_id.cmp(&right.planned_workout_id))
            });
            Ok(workouts)
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, PlannedWorkoutError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().expect("planned workout repo mutex poisoned");
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.planned_workout_id == workout.planned_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}
