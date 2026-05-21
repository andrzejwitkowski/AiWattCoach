use crate::domain::{
    external_sync::ExternalSyncStateRepository,
    identity::Clock,
    settings::{UserSettingsUseCases, DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL},
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorBatchRequest,
        TrainingPlanSupervisorOperation, TrainingPlanSupervisorScheduler,
        TrainingPlanSupervisorStatus,
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
        _original_plan: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        Box::pin(async { Ok(None) })
    }
}

impl<Repo, Settings, Time, Batch, SyncStates> TrainingPlanSupervisorScheduler
    for TrainingPlanSupervisorService<Repo, Settings, Time, Batch, SyncStates>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    fn initialize_pending_review(
        &self,
        user_id: &str,
        worker_operation_key: &str,
        worker_saved_at_epoch_seconds: i64,
        original_plan: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        let repository = self.repository.clone();
        let settings = self.settings.clone();
        let clock = self.clock.clone();
        let batch = self.batch.clone();
        let user_id = user_id.to_string();
        let worker_operation_key = worker_operation_key.to_string();
        let original_plan = original_plan.to_string();
        Box::pin(async move {
            if let Some(existing) = repository
                .find_by_worker_operation_key(&worker_operation_key)
                .await?
            {
                if existing.status == TrainingPlanSupervisorStatus::Pending
                    && existing.batch_name.is_none()
                {
                    return submit_batch_for_operation(
                        repository,
                        batch,
                        existing,
                        gemini_api_key(&settings, &user_id).await?,
                        original_plan,
                        clock.now_epoch_seconds(),
                    )
                    .await
                    .map(Some);
                }
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
            let gemini_api_key = settings
                .ai_agents
                .gemini_api_key
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    TrainingPlanError::Unavailable(
                        "Gemini supervisor requires a configured Gemini API key".to_string(),
                    )
                })?;

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
            let operation = repository.upsert(operation).await?;
            submit_batch_for_operation(
                repository,
                batch,
                operation,
                gemini_api_key,
                original_plan,
                clock.now_epoch_seconds(),
            )
            .await
            .map(Some)
        })
    }
}

async fn submit_batch_for_operation<Repo, Batch>(
    repository: Repo,
    batch: Batch,
    operation: TrainingPlanSupervisorOperation,
    gemini_api_key: String,
    original_plan: String,
    now_epoch_seconds: i64,
) -> Result<TrainingPlanSupervisorOperation, TrainingPlanError>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
{
    let submission = batch
        .submit_review(
            &gemini_api_key,
            TrainingPlanSupervisorBatchRequest {
                user_id: operation.user_id.clone(),
                worker_operation_key: operation.worker_operation_key.clone(),
                model: operation.model.clone(),
                original_plan,
            },
        )
        .await?;
    repository
        .upsert(operation.with_batch_submission(submission.batch_name, now_epoch_seconds))
        .await
}

async fn gemini_api_key<Settings>(
    settings: &Settings,
    user_id: &str,
) -> Result<String, TrainingPlanError>
where
    Settings: UserSettingsUseCases + Clone + 'static,
{
    let user_settings = settings
        .get_settings(user_id)
        .await
        .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
    user_settings
        .ai_agents
        .gemini_api_key
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            TrainingPlanError::Unavailable(
                "Gemini supervisor requires a configured Gemini API key".to_string(),
            )
        })
}
