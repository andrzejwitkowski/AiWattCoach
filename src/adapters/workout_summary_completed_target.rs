use crate::domain::{
    completed_workouts::{canonical_completed_workout_id, CompletedWorkoutRepository},
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
            Ok(
                resolve_completed_workout(&repository, &user_id, &workout_id)
                    .await?
                    .map(|workout| {
                        let completed_workout_id = workout.completed_workout_id;
                        let preferred_workout_id = workout
                            .source_activity_id
                            .unwrap_or_else(|| completed_workout_id.clone());
                        let mut equivalent_workout_ids = vec![preferred_workout_id.clone()];
                        if !equivalent_workout_ids.contains(&completed_workout_id) {
                            equivalent_workout_ids.push(completed_workout_id);
                        }

                        ResolvedCompletedWorkoutTarget {
                            preferred_workout_id,
                            equivalent_workout_ids,
                        }
                    }),
            )
        })
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
