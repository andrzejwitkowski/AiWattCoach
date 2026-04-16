use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{CompletedWorkout, CompletedWorkoutError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait CompletedWorkoutRepository: Clone + Send + Sync + 'static {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>;

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>;

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>;

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>;

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>>;
}

impl CompletedWorkoutRepository for () {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        _user_id: &str,
        _source_activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_latest_by_user_id(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        Box::pin(async move { Ok(workout) })
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

#[cfg(test)]
impl CompletedWorkoutRepository for NoopCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
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
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
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
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
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
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
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
            workouts.sort_by(|left, right| {
                left.start_date_local
                    .cmp(&right.start_date_local)
                    .then_with(|| left.completed_workout_id.cmp(&right.completed_workout_id))
            });
            Ok(workouts)
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let stored = stored
                .lock()
                .expect("completed workout repo mutex poisoned");
            let mut workouts = stored
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                left.start_date_local
                    .cmp(&right.start_date_local)
                    .then_with(|| left.completed_workout_id.cmp(&right.completed_workout_id))
            });
            Ok(workouts)
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
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
