use crate::domain::{
    identity::Clock,
    settings::{UserSettingsUseCases, DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL},
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorOperation, TrainingPlanSupervisorScheduler,
    },
};

use super::{TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorService};

#[derive(Clone, Default)]
pub struct NoopTrainingPlanSupervisorScheduler;

impl TrainingPlanSupervisorScheduler for NoopTrainingPlanSupervisorScheduler {
    fn initialize_pending_review(
        &self,
        _user_id: &str,
        _worker_operation_key: &str,
        _worker_saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        Box::pin(async { Ok(None) })
    }
}

impl<Repo, Settings, Time> TrainingPlanSupervisorScheduler
    for TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    fn initialize_pending_review(
        &self,
        user_id: &str,
        worker_operation_key: &str,
        worker_saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        let repository = self.repository.clone();
        let settings = self.settings.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            if let Some(existing) = repository
                .find_by_worker_operation_key(&worker_operation_key)
                .await?
            {
                return Ok(Some(existing));
            }

            let settings = settings
                .find_settings(&user_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let Some(settings) = settings else {
                return Ok(None);
            };
            if !settings.ai_agents.training_plan_supervisor_enabled {
                return Ok(None);
            }

            let model = settings
                .ai_agents
                .training_plan_supervisor_model
                .clone()
                .unwrap_or_else(|| DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL.to_string());
            let operation = TrainingPlanSupervisorOperation::pending(
                worker_operation_key,
                user_id,
                worker_saved_at_epoch_seconds,
                model,
                clock.now_epoch_seconds(),
            );
            repository.upsert(operation).await.map(Some)
        })
    }
}
