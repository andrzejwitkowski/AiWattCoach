use std::{future::Future, pin::Pin};

use super::CompletedWorkout;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait CompletedWorkoutRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, std::convert::Infallible>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopCompletedWorkoutRepository;

#[cfg(test)]
impl CompletedWorkoutRepository for NoopCompletedWorkoutRepository {
    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, std::convert::Infallible>> {
        Box::pin(async move { Ok(workout) })
    }
}
