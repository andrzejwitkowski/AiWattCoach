use crate::domain::{
    intervals::ActivityRepositoryPort,
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
    Repo: ActivityRepositoryPort + Clone + Send + Sync + 'static,
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
            repository
                .find_by_user_id_and_activity_id(&user_id, &workout_id)
                .await
                .map(|activity| activity.is_some())
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
        })
    }
}
