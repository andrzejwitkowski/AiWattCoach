use std::{future::Future, pin::Pin};

use super::PlannedWorkout;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedWorkoutRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, std::convert::Infallible>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopPlannedWorkoutRepository;

#[cfg(test)]
impl PlannedWorkoutRepository for NoopPlannedWorkoutRepository {
    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, std::convert::Infallible>> {
        Box::pin(async move { Ok(workout) })
    }
}
