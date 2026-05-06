use super::{
    compute_power_curve, BoxFuture, CompletedWorkout, CompletedWorkoutError,
    CompletedWorkoutPowerCurve, CompletedWorkoutRepository,
};

/// Decorator that automatically computes and attaches the 5-second power curve
/// to completed workouts on [`upsert`](CompletedWorkoutRepository::upsert).
///
/// The curve is computed only when `power_curve_5s` is `None` and workout
/// details are available. Other repository methods delegate directly to the
/// inner repository.
#[derive(Clone)]
pub struct PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    inner: Repo,
}

impl<Repo> PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    pub fn new(inner: Repo) -> Self {
        Self { inner }
    }
}

impl<Repo> CompletedWorkoutRepository for PowerCurveCompletedWorkoutRepository<Repo>
where
    Repo: CompletedWorkoutRepository,
{
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .find_by_user_id_and_completed_workout_id(user_id, completed_workout_id)
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .find_by_user_id_and_source_activity_id(user_id, source_activity_id)
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner.find_latest_by_user_id(user_id)
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner.list_by_user_id(user_id)
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        self.inner
            .list_by_user_id_and_date_range(user_id, oldest, newest)
    }

    fn upsert(
        &self,
        mut workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            if workout.power_curve_5s.is_none() && workout.details_unavailable_reason.is_none() {
                if let Ok(curve) = compute_power_curve(&workout, 5) {
                    workout.power_curve_5s = Some(curve);
                }
            }
            inner.upsert(workout).await
        })
    }

    fn set_power_curve_5s_if_missing(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        curve: CompletedWorkoutPowerCurve,
    ) -> BoxFuture<Result<(), CompletedWorkoutError>> {
        self.inner
            .set_power_curve_5s_if_missing(user_id, completed_workout_id, curve)
    }
}
