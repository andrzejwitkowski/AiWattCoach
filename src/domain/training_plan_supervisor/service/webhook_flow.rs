use crate::domain::{
    external_sync::ExternalSyncStateRepository,
    identity::Clock,
    settings::UserSettingsUseCases,
    training_plan::{TrainingPlanError, TrainingPlanProjectionRepository},
    training_plan_supervisor::{
        BoxFuture, GeminiSupervisorWebhookOutcome, TrainingPlanSupervisorBatchPort,
        TrainingPlanSupervisorOperationRepository,
    },
};

use super::TrainingPlanSupervisorService;

pub(super) struct GeminiBatchWebhookInput {
    pub(super) worker_operation_key: String,
    pub(super) provided_webhook_token: String,
    pub(super) expected_webhook_token: Option<String>,
    pub(super) event_type: String,
    pub(super) batch_name: String,
}

impl<Repo, Settings, Time, ServiceBatch, SyncStates>
    TrainingPlanSupervisorService<Repo, Settings, Time, ServiceBatch, SyncStates>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    ServiceBatch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    pub(super) fn handle_gemini_batch_webhook<Projections, Batch>(
        &self,
        projections: Projections,
        batch: Batch,
        input: GeminiBatchWebhookInput,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
        Batch: TrainingPlanSupervisorBatchPort + Clone,
    {
        let settings = self.settings.clone();
        let service = self.clone();
        Box::pin(async move {
            let Some(expected_webhook_token) = input
                .expected_webhook_token
                .filter(|token| !token.trim().is_empty())
            else {
                return Err(TrainingPlanError::GeminiSupervisorWebhookNotConfigured);
            };
            if expected_webhook_token != input.provided_webhook_token {
                return Err(TrainingPlanError::GeminiSupervisorWebhookUnauthorized);
            }
            if input.event_type != "batch.succeeded" {
                return Ok(GeminiSupervisorWebhookOutcome::Ignored);
            }

            let operation = service
                .repository
                .find_by_worker_operation_key(&input.worker_operation_key)
                .await?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {} not found",
                        input.worker_operation_key
                    ))
                })?;
            let user_settings = settings
                .get_settings(&operation.user_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let gemini_api_key = user_settings
                .ai_agents
                .gemini_api_key
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    TrainingPlanError::Unavailable(
                        "Gemini supervisor webhook requires a configured Gemini API key"
                            .to_string(),
                    )
                })?;
            let review = batch
                .download_result(&gemini_api_key, &input.batch_name)
                .await?;
            let completed = service
                .complete_review(projections, &input.worker_operation_key, review)
                .await?;
            Ok(GeminiSupervisorWebhookOutcome::Accepted(Box::new(
                completed,
            )))
        })
    }
}
