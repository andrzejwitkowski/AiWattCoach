use std::{future::Future, pin::Pin};

use super::{PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedCompletedWorkoutLinkRepository: Clone + Send + Sync + 'static {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>>;

    fn find_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>>;

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> BoxFuture<Result<PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError>>;
}
