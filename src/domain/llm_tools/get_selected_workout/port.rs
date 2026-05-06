use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutError, CompletedWorkoutPowerCurve},
    planned_workouts::{PlannedWorkout, PlannedWorkoutError},
    races::{Race, RaceError},
    workout_summary::{WorkoutSummary, WorkoutSummaryError},
};

/// Object-safe port for loading workout data needed by `get_selected_workout`.
pub trait GetSelectedWorkoutDataPort: Send + Sync {
    fn list_completed_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    >;

    fn list_planned_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>;

    fn list_races_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, RaceError>>;

    fn find_summaries_by_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>;

    fn persist_power_curve_5s_if_missing(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
        _curve: CompletedWorkoutPowerCurve,
    ) -> crate::domain::completed_workouts::BoxFuture<Result<(), CompletedWorkoutError>> {
        Box::pin(async { Ok(()) })
    }
}
