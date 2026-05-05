use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutError, CompletedWorkoutRepository},
    planned_workouts::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository},
    races::{Race, RaceError, RaceRepository},
    workout_summary::{WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository},
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
}

/// Adapts concrete repositories to the object-safe tool data port.
pub struct GetSelectedWorkoutDataAdapter<Completed, Planned, Races, Summaries>
where
    Completed: CompletedWorkoutRepository,
    Planned: PlannedWorkoutRepository,
    Races: RaceRepository,
    Summaries: WorkoutSummaryRepository,
{
    pub completed: Completed,
    pub planned: Planned,
    pub races: Races,
    pub summaries: Summaries,
}

impl<Completed, Planned, Races, Summaries> GetSelectedWorkoutDataPort
    for GetSelectedWorkoutDataAdapter<Completed, Planned, Races, Summaries>
where
    Completed: CompletedWorkoutRepository,
    Planned: PlannedWorkoutRepository,
    Races: RaceRepository,
    Summaries: WorkoutSummaryRepository,
{
    fn list_completed_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        self.completed
            .list_by_user_id_and_date_range(user_id, oldest, newest)
    }

    fn list_planned_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>
    {
        self.planned
            .list_by_user_id_and_date_range(user_id, oldest, newest)
    }

    fn list_races_by_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, RaceError>> {
        self.races.list_by_user_id_and_range(
            user_id,
            &crate::domain::intervals::DateRange {
                oldest: oldest.to_string(),
                newest: newest.to_string(),
            },
        )
    }

    fn find_summaries_by_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>
    {
        self.summaries
            .find_by_user_id_and_workout_ids(user_id, workout_ids)
    }
}
