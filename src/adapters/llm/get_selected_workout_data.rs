use crate::domain::{
    completed_workouts::{
        canonical_completed_workout_id, CompletedWorkout, CompletedWorkoutError,
        CompletedWorkoutPowerCurve, CompletedWorkoutRepository,
    },
    llm_tools::GetSelectedWorkoutDataPort,
    planned_workouts::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository},
    races::{Race, RaceError, RaceRepository},
    workout_summary::{WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository},
};

use crate::domain::llm_tools::SelectedWorkoutData;

/// Adapts concrete repositories to the object-safe tool data port.
pub struct GetSelectedWorkoutDataAdapter<Completed, Planned, Races, Summaries>
where
    Completed: CompletedWorkoutRepository,
    Planned: PlannedWorkoutRepository,
    Races: RaceRepository + Clone,
    Summaries: WorkoutSummaryRepository + Clone,
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
    Races: RaceRepository + Clone,
    Summaries: WorkoutSummaryRepository + Clone,
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

    fn load_selected_workout_data_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<Result<SelectedWorkoutData, WorkoutSummaryError>>
    {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let completed_repo = self.completed.clone();
        let planned_repo = self.planned.clone();
        let races_repo = self.races.clone();
        let summary_repo = self.summaries.clone();

        Box::pin(async move {
            if let Some(completed) =
                load_completed_by_frontend_id(&completed_repo, &user_id, &workout_id)
                    .await
                    .map_err(|err| WorkoutSummaryError::Repository(err.to_string()))?
            {
                let date = completed
                    .start_date_local
                    .get(..10)
                    .unwrap_or_default()
                    .to_string();
                let planned = planned_repo
                    .list_by_user_id_and_date_range(&user_id, &date, &date)
                    .await
                    .map_err(|err| WorkoutSummaryError::Repository(err.to_string()))?;
                let races = races_repo
                    .list_by_user_id_and_range(
                        &user_id,
                        &crate::domain::intervals::DateRange {
                            oldest: date.clone(),
                            newest: date.clone(),
                        },
                    )
                    .await
                    .map_err(|err: RaceError| WorkoutSummaryError::Repository(err.to_string()))?;
                let summary_ids = summary_lookup_ids_for_completed(&completed);
                let summaries = if summary_ids.is_empty() {
                    Vec::new()
                } else {
                    summary_repo
                        .find_by_user_id_and_workout_ids(&user_id, summary_ids)
                        .await?
                };

                return Ok(SelectedWorkoutData {
                    completed: vec![completed],
                    planned,
                    races,
                    summaries,
                });
            }

            if let Some(planned) = load_planned_by_frontend_id(&planned_repo, &user_id, &workout_id)
                .await
                .map_err(|err| WorkoutSummaryError::Repository(err.to_string()))?
            {
                let races = races_repo
                    .list_by_user_id_and_range(
                        &user_id,
                        &crate::domain::intervals::DateRange {
                            oldest: planned.date.clone(),
                            newest: planned.date.clone(),
                        },
                    )
                    .await
                    .map_err(|err: RaceError| WorkoutSummaryError::Repository(err.to_string()))?;

                return Ok(SelectedWorkoutData {
                    completed: Vec::new(),
                    planned: vec![planned],
                    races,
                    summaries: Vec::new(),
                });
            }

            if let Some(race) = races_repo
                .find_by_user_id_and_race_id(&user_id, &workout_id)
                .await
                .map_err(|err: RaceError| WorkoutSummaryError::Repository(err.to_string()))?
            {
                return Ok(SelectedWorkoutData {
                    completed: Vec::new(),
                    planned: Vec::new(),
                    races: vec![race],
                    summaries: Vec::new(),
                });
            }

            Ok(SelectedWorkoutData {
                completed: Vec::new(),
                planned: Vec::new(),
                races: Vec::new(),
                summaries: Vec::new(),
            })
        })
    }

    fn persist_power_curve_5s_if_missing(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        curve: CompletedWorkoutPowerCurve,
    ) -> crate::domain::completed_workouts::BoxFuture<Result<(), CompletedWorkoutError>> {
        self.completed
            .set_power_curve_5s_if_missing(user_id, completed_workout_id, curve)
    }
}

async fn load_completed_by_frontend_id<Completed>(
    repository: &Completed,
    user_id: &str,
    workout_id: &str,
) -> Result<Option<CompletedWorkout>, CompletedWorkoutError>
where
    Completed: CompletedWorkoutRepository,
{
    if let Some(workout) = repository
        .find_by_user_id_and_source_activity_id(user_id, workout_id)
        .await?
    {
        return Ok(Some(workout));
    }

    if let Some(workout) = repository
        .find_by_user_id_and_completed_workout_id(
            user_id,
            &canonical_completed_workout_id(workout_id),
        )
        .await?
    {
        return Ok(Some(workout));
    }

    let workouts = repository.list_by_user_id(user_id).await?;
    Ok(workouts.into_iter().find(|workout| {
        workout.external_id.as_deref() == Some(workout_id)
            || workout.planned_workout_id.as_deref() == Some(workout_id)
    }))
}

async fn load_planned_by_frontend_id<Planned>(
    repository: &Planned,
    user_id: &str,
    workout_id: &str,
) -> Result<Option<PlannedWorkout>, PlannedWorkoutError>
where
    Planned: PlannedWorkoutRepository,
{
    let planned = repository.list_by_user_id(user_id).await?;
    Ok(planned
        .into_iter()
        .find(|workout| workout.planned_workout_id == workout_id))
}

fn summary_lookup_ids_for_completed(workout: &CompletedWorkout) -> Vec<String> {
    let mut ids = vec![workout.completed_workout_id.clone()];

    if let Some(source_activity_id) = workout.source_activity_id.as_ref() {
        if !ids.contains(source_activity_id) {
            ids.push(source_activity_id.clone());
        }
    }

    if let Some(external_id) = workout.external_id.as_ref() {
        if !ids.contains(external_id) {
            ids.push(external_id.clone());
        }
    }

    ids
}
