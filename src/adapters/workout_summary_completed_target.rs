use crate::domain::{
    completed_workouts::{
        canonical_completed_workout_id, completed_workout_activity_id, CompletedWorkout,
        CompletedWorkoutRepository,
    },
    workout_summary::{
        BoxFuture, CompletedWorkoutTargetUseCases, ResolvedCompletedWorkoutTarget,
        WorkoutSummaryError,
    },
};

#[derive(Clone)]
pub struct CompletedWorkoutTargetAdapter<Repo> {
    repository: Repo,
}

impl<Repo> CompletedWorkoutTargetAdapter<Repo> {
    pub fn new(repository: Repo) -> Self {
        Self { repository }
    }
}

impl<Repo> CompletedWorkoutTargetUseCases for CompletedWorkoutTargetAdapter<Repo>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    fn is_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<bool, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let resolved = resolve_completed_workout(&repository, &user_id, &workout_id).await?;
            Ok(resolved.is_some())
        })
    }

    fn resolve_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let Some(workout) =
                resolve_completed_workout(&repository, &user_id, &workout_id).await?
            else {
                return Ok(None);
            };

            let mut equivalent_workout_ids =
                equivalent_workout_ids_for_workout(&repository, &user_id, &workout).await?;
            let preferred_workout_id = workout
                .source_activity_id
                .clone()
                .unwrap_or_else(|| workout.completed_workout_id.clone());
            push_unique_workout_id(&mut equivalent_workout_ids, preferred_workout_id.clone());

            Ok(Some(ResolvedCompletedWorkoutTarget {
                preferred_workout_id,
                equivalent_workout_ids,
            }))
        })
    }
}

async fn equivalent_workout_ids_for_workout<Repo>(
    repository: &Repo,
    user_id: &str,
    workout: &CompletedWorkout,
) -> Result<Vec<String>, WorkoutSummaryError>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    let mut equivalent_workout_ids = Vec::new();
    push_unique_workout_id(
        &mut equivalent_workout_ids,
        workout
            .source_activity_id
            .clone()
            .unwrap_or_else(|| workout.completed_workout_id.clone()),
    );
    push_unique_workout_id(
        &mut equivalent_workout_ids,
        workout.completed_workout_id.clone(),
    );

    let siblings = repository
        .list_by_user_id(user_id)
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
    for sibling in siblings {
        if same_completed_workout_family(workout, &sibling) {
            push_unique_workout_id(
                &mut equivalent_workout_ids,
                sibling
                    .source_activity_id
                    .clone()
                    .unwrap_or_else(|| sibling.completed_workout_id.clone()),
            );
            push_unique_workout_id(&mut equivalent_workout_ids, sibling.completed_workout_id);
        }
    }

    Ok(equivalent_workout_ids)
}

fn same_completed_workout_family(left: &CompletedWorkout, right: &CompletedWorkout) -> bool {
    if left.user_id != right.user_id {
        return false;
    }

    if left.completed_workout_id == right.completed_workout_id {
        return true;
    }

    if same_non_empty_option(
        left.planned_workout_id.as_deref(),
        right.planned_workout_id.as_deref(),
    ) {
        return true;
    }

    same_non_empty_option(left.external_id.as_deref(), right.external_id.as_deref())
        || completed_workout_activity_id(&left.completed_workout_id)
            == completed_workout_activity_id(&right.completed_workout_id)
}

fn same_non_empty_option(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => !left.is_empty() && left == right,
        _ => false,
    }
}

fn push_unique_workout_id(equivalent_workout_ids: &mut Vec<String>, workout_id: String) {
    if !equivalent_workout_ids.contains(&workout_id) {
        equivalent_workout_ids.push(workout_id);
    }
}

async fn resolve_completed_workout<Repo>(
    repository: &Repo,
    user_id: &str,
    workout_id: &str,
) -> Result<Option<crate::domain::completed_workouts::CompletedWorkout>, WorkoutSummaryError>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    match repository
        .find_by_user_id_and_source_activity_id(user_id, workout_id)
        .await
    {
        Ok(Some(workout)) => Ok(Some(workout)),
        Ok(None) => repository
            .find_by_user_id_and_completed_workout_id(
                user_id,
                &canonical_completed_workout_id(workout_id),
            )
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string())),
        Err(error) => Err(WorkoutSummaryError::Repository(error.to_string())),
    }
}
