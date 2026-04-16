use super::{BoxFuture, CompletedWorkout, CompletedWorkoutError, CompletedWorkoutRepository};

pub trait CompletedWorkoutReadUseCases: Send + Sync {
    fn list_completed_workouts(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>;

    fn get_completed_workout(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackfillCompletedWorkoutDetailsResult {
    pub scanned: usize,
    pub enriched: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub trait CompletedWorkoutAdminUseCases: Send + Sync {
    fn backfill_missing_details(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<BackfillCompletedWorkoutDetailsResult, CompletedWorkoutError>>;
}

#[derive(Clone)]
pub struct CompletedWorkoutReadService<Repo> {
    repository: Repo,
}

impl<Repo> CompletedWorkoutReadService<Repo> {
    pub fn new(repository: Repo) -> Self {
        Self { repository }
    }
}

impl<Repo> CompletedWorkoutReadUseCases for CompletedWorkoutReadService<Repo>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    fn list_completed_workouts(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
        })
    }

    fn get_completed_workout(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            match repository
                .find_by_user_id_and_source_activity_id(&user_id, &activity_id)
                .await
            {
                Ok(Some(workout)) => Ok(Some(workout)),
                Ok(None) => {
                    repository
                        .find_by_user_id_and_completed_workout_id(
                            &user_id,
                            &canonical_completed_workout_id(&activity_id),
                        )
                        .await
                }
                Err(error) => Err(error),
            }
        })
    }
}

pub fn canonical_completed_workout_id(activity_id: &str) -> String {
    if activity_id.starts_with("intervals-activity:") {
        activity_id.to_string()
    } else {
        format!("intervals-activity:{activity_id}")
    }
}

pub fn completed_workout_activity_id(id: &str) -> &str {
    id.strip_prefix("intervals-activity:").unwrap_or(id)
}
