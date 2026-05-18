mod completion;
mod scheduler;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;
mod webhook;
mod webhook_flow;

use crate::domain::{
    external_sync::NoopExternalSyncStateRepository, identity::Clock, settings::UserSettingsUseCases,
};

use super::TrainingPlanSupervisorOperationRepository;

pub use scheduler::NoopTrainingPlanSupervisorScheduler;
pub use webhook::{
    GeminiTrainingPlanSupervisorWebhookService, TrainingPlanSupervisorWebhookUseCases,
};

#[derive(Clone)]
pub struct TrainingPlanSupervisorService<
    Repo,
    Settings,
    Time,
    Batch = NoopTrainingPlanSupervisorBatch,
    SyncStates = NoopExternalSyncStateRepository,
> where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Batch: crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository + Clone,
{
    repository: Repo,
    settings: Settings,
    clock: Time,
    batch: Batch,
    sync_states: SyncStates,
}

impl<Repo, Settings, Time> TrainingPlanSupervisorService<Repo, Settings, Time>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
{
    pub fn new(repository: Repo, settings: Settings, clock: Time) -> Self {
        Self {
            repository,
            settings,
            clock,
            batch: NoopTrainingPlanSupervisorBatch,
            sync_states: NoopExternalSyncStateRepository,
        }
    }
}

impl<Repo, Settings, Time, Batch, SyncStates>
    TrainingPlanSupervisorService<Repo, Settings, Time, Batch, SyncStates>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Batch: crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository + Clone,
{
    pub fn with_batch<NextBatch>(
        self,
        batch: NextBatch,
    ) -> TrainingPlanSupervisorService<Repo, Settings, Time, NextBatch, SyncStates>
    where
        NextBatch: crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchPort + Clone,
    {
        TrainingPlanSupervisorService {
            repository: self.repository,
            settings: self.settings,
            clock: self.clock,
            batch,
            sync_states: self.sync_states,
        }
    }

    pub fn with_sync_states<NextSyncStates>(
        self,
        sync_states: NextSyncStates,
    ) -> TrainingPlanSupervisorService<Repo, Settings, Time, Batch, NextSyncStates>
    where
        NextSyncStates: crate::domain::external_sync::ExternalSyncStateRepository + Clone,
    {
        TrainingPlanSupervisorService {
            repository: self.repository,
            settings: self.settings,
            clock: self.clock,
            batch: self.batch,
            sync_states,
        }
    }
}

#[derive(Clone, Default)]
pub struct NoopTrainingPlanSupervisorBatch;

impl crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchPort
    for NoopTrainingPlanSupervisorBatch
{
    fn submit_review(
        &self,
        _api_key: &str,
        _request: crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchRequest,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<
            crate::domain::training_plan_supervisor::TrainingPlanSupervisorBatchSubmission,
            crate::domain::training_plan::TrainingPlanError,
        >,
    > {
        Box::pin(async {
            Err(
                crate::domain::training_plan::TrainingPlanError::Unavailable(
                    "training plan supervisor batch client is not configured".to_string(),
                ),
            )
        })
    }

    fn download_result(
        &self,
        _api_key: &str,
        _batch_name: &str,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<
            crate::domain::training_plan_supervisor::TrainingPlanSupervisorReview,
            crate::domain::training_plan::TrainingPlanError,
        >,
    > {
        Box::pin(async {
            Err(
                crate::domain::training_plan::TrainingPlanError::Unavailable(
                    "training plan supervisor batch client is not configured".to_string(),
                ),
            )
        })
    }
}
