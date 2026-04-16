use crate::domain::{
    completed_workouts::{canonical_completed_workout_id, CompletedWorkoutRepository},
    workout_summary::{BoxFuture, CompletedWorkoutTargetUseCases, WorkoutSummaryError},
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
            match repository
                .find_by_user_id_and_source_activity_id(&user_id, &workout_id)
                .await
            {
                Ok(Some(_)) => Ok(true),
                Ok(None) => repository
                    .find_by_user_id_and_completed_workout_id(
                        &user_id,
                        &canonical_completed_workout_id(&workout_id),
                    )
                    .await
                    .map(|workout| workout.is_some())
                    .map_err(|error| WorkoutSummaryError::Repository(error.to_string())),
                Err(error) => Err(WorkoutSummaryError::Repository(error.to_string())),
            }
        })
    }
}
