use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    external_sync::ExternalSyncStateRepository,
    identity::Clock,
    settings::UserSettingsUseCases,
    training_plan::{TrainingPlanError, TrainingPlanProjectionRepository},
    training_plan_supervisor::{
        BoxFuture, GeminiSupervisorWebhookOutcome, TrainingPlanSupervisorBatchPort,
        TrainingPlanSupervisorOperationRepository,
    },
};

use super::{webhook_flow::GeminiBatchWebhookInput, TrainingPlanSupervisorService};

pub trait TrainingPlanSupervisorWebhookUseCases: Send + Sync {
    fn receive_gemini_batch_webhook(
        &self,
        worker_operation_key: &str,
        provided_webhook_token: &str,
        event_type: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>>;
}

#[derive(Clone)]
pub struct GeminiTrainingPlanSupervisorWebhookService<
    Repo,
    Settings,
    Time,
    Projections,
    Batch,
    SyncStates,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    supervisor: TrainingPlanSupervisorService<Repo, Settings, Time, Batch, SyncStates>,
    projections: Projections,
    batch: Batch,
    webhook_token: Option<String>,
    refresh: Refresh,
}

impl<Repo, Settings, Time, Projections, Batch, SyncStates>
    GeminiTrainingPlanSupervisorWebhookService<Repo, Settings, Time, Projections, Batch, SyncStates>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    pub fn new(
        supervisor: TrainingPlanSupervisorService<Repo, Settings, Time, Batch, SyncStates>,
        projections: Projections,
        batch: Batch,
        webhook_token: Option<String>,
    ) -> Self {
        Self {
            supervisor,
            projections,
            batch,
            webhook_token,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Repo, Settings, Time, Projections, Batch, SyncStates, Refresh>
    GeminiTrainingPlanSupervisorWebhookService<
        Repo,
        Settings,
        Time,
        Projections,
        Batch,
        SyncStates,
        Refresh,
    >
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    pub fn with_calendar_view_refresh<NextRefresh>(
        self,
        refresh: NextRefresh,
    ) -> GeminiTrainingPlanSupervisorWebhookService<
        Repo,
        Settings,
        Time,
        Projections,
        Batch,
        SyncStates,
        NextRefresh,
    >
    where
        NextRefresh: CalendarEntryViewRefreshPort + Clone,
    {
        GeminiTrainingPlanSupervisorWebhookService {
            supervisor: self.supervisor,
            projections: self.projections,
            batch: self.batch,
            webhook_token: self.webhook_token,
            refresh,
        }
    }
}

impl<Repo, Settings, Time, Projections, Batch, SyncStates, Refresh>
    TrainingPlanSupervisorWebhookUseCases
    for GeminiTrainingPlanSupervisorWebhookService<
        Repo,
        Settings,
        Time,
        Projections,
        Batch,
        SyncStates,
        Refresh,
    >
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Projections: TrainingPlanProjectionRepository + Clone,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    fn receive_gemini_batch_webhook(
        &self,
        worker_operation_key: &str,
        provided_webhook_token: &str,
        event_type: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>> {
        self.supervisor.handle_gemini_batch_webhook(
            self.projections.clone(),
            self.batch.clone(),
            self.refresh.clone(),
            GeminiBatchWebhookInput {
                worker_operation_key: worker_operation_key.to_string(),
                provided_webhook_token: provided_webhook_token.to_string(),
                expected_webhook_token: self.webhook_token.clone(),
                event_type: event_type.to_string(),
                batch_name: batch_name.to_string(),
            },
        )
    }
}
