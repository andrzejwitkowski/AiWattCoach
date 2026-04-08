use crate::domain::{
    intervals::ActivityRepositoryPort,
    workout_summary::{BoxFuture, LatestCompletedActivityUseCases, WorkoutSummaryError},
};

#[derive(Clone)]
pub struct LatestCompletedActivityAdapter<Repo> {
    repository: Repo,
}

impl<Repo> LatestCompletedActivityAdapter<Repo> {
    pub fn new(repository: Repo) -> Self {
        Self { repository }
    }
}

impl<Repo> LatestCompletedActivityUseCases for LatestCompletedActivityAdapter<Repo>
where
    Repo: ActivityRepositoryPort + Clone + Send + Sync + 'static,
{
    fn latest_completed_activity_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<String>, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            repository
                .find_latest_by_user_id(&user_id)
                .await
                .map(|activity| activity.map(|activity| activity.id))
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
        })
    }
}
