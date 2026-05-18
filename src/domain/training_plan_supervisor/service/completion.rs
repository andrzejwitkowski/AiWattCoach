use crate::domain::{
    identity::Clock,
    settings::UserSettingsUseCases,
    training_plan::{TrainingPlanError, TrainingPlanProjectionRepository},
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
        TrainingPlanSupervisorReview,
    },
};

use super::TrainingPlanSupervisorService;

impl<Repo, Settings, Time> TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    pub fn complete_review<Projections>(
        &self,
        projections: Projections,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
    {
        let repository = self.repository.clone();
        let worker_operation_key = worker_operation_key.to_string();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let existing = repository
                .find_by_worker_operation_key(&worker_operation_key)
                .await?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;
            let completed = existing.complete_review(review, now_epoch_seconds)?;
            let completed = repository.upsert(completed).await?;
            projections
                .update_supervisor_status(
                    &completed.user_id,
                    &completed.worker_operation_key,
                    Some(completed.status),
                    completed.updated_at_epoch_seconds,
                )
                .await?;
            Ok(completed)
        })
    }
}
