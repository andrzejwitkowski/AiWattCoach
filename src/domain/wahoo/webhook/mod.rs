use crate::domain::{
    external_sync::{ExternalImportError, ExternalImportOutcome, ExternalImportUseCases},
    identity::Clock,
    settings::{SettingsError, UserSettingsRepository},
    training_load::{TrainingLoadError, TrainingLoadRecomputeUseCases},
    wahoo_fit_enrichment::{WahooFitEnrichmentError, WahooFitEnrichmentQueueUseCases},
};

use super::{map_workout_to_import_command, BoxFuture, WahooError, WahooUseCases, WahooWorkout};

const DEFAULT_MANUAL_SYNC_PER_PAGE: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooWebhookAccepted {
    pub user_id: String,
    pub completed_workout_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooWebhookOutcome {
    Ignored,
    Accepted(WahooWebhookAccepted),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualWahooSyncResult {
    pub scanned: usize,
    pub imported: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooWebhookError {
    NotConfigured,
    Unauthorized,
    InvalidPayload(String),
    Settings(String),
    Import(String),
    Queue(String),
    TrainingLoad(String),
    Wahoo(String),
}

impl std::fmt::Display for WahooWebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "Wahoo webhook is not configured"),
            Self::Unauthorized => write!(f, "Wahoo webhook token is invalid"),
            Self::InvalidPayload(message)
            | Self::Settings(message)
            | Self::Import(message)
            | Self::Queue(message)
            | Self::TrainingLoad(message)
            | Self::Wahoo(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WahooWebhookError {}

pub trait WahooWebhookUseCases: Send + Sync {
    fn import_webhook_workout(
        &self,
        webhook_token: &str,
        wahoo_user_id: i64,
        workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>>;

    fn sync_completed_workouts_for_user(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<ManualWahooSyncResult, WahooWebhookError>>;
}

impl<T> WahooWebhookUseCases for std::sync::Arc<T>
where
    T: WahooWebhookUseCases + ?Sized,
{
    fn import_webhook_workout(
        &self,
        webhook_token: &str,
        wahoo_user_id: i64,
        workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>> {
        self.as_ref()
            .import_webhook_workout(webhook_token, wahoo_user_id, workout)
    }

    fn sync_completed_workouts_for_user(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<ManualWahooSyncResult, WahooWebhookError>> {
        self.as_ref().sync_completed_workouts_for_user(user_id)
    }
}

#[derive(Clone)]
pub struct WahooWebhookService<SettingsRepo, Imports, Wahoo, TrainingLoad, Queue, Time>
where
    SettingsRepo: UserSettingsRepository + Clone + 'static,
    Imports: ExternalImportUseCases + Clone + 'static,
    Wahoo: WahooUseCases + Clone + 'static,
    TrainingLoad: TrainingLoadRecomputeUseCases + Clone + 'static,
    Queue: WahooFitEnrichmentQueueUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    settings_repository: SettingsRepo,
    imports: Imports,
    wahoo_service: Wahoo,
    training_load: TrainingLoad,
    queue: Queue,
    clock: Time,
    webhook_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ImportedWorkout {
    completed_workout_id: String,
    oldest_date: Option<String>,
}

impl<SettingsRepo, Imports, Wahoo, TrainingLoad, Queue, Time>
    WahooWebhookService<SettingsRepo, Imports, Wahoo, TrainingLoad, Queue, Time>
where
    SettingsRepo: UserSettingsRepository + Clone + 'static,
    Imports: ExternalImportUseCases + Clone + 'static,
    Wahoo: WahooUseCases + Clone + 'static,
    TrainingLoad: TrainingLoadRecomputeUseCases + Clone + 'static,
    Queue: WahooFitEnrichmentQueueUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    pub fn new(
        settings_repository: SettingsRepo,
        imports: Imports,
        wahoo_service: Wahoo,
        training_load: TrainingLoad,
        queue: Queue,
        clock: Time,
        webhook_token: Option<String>,
    ) -> Self {
        Self {
            settings_repository,
            imports,
            wahoo_service,
            training_load,
            queue,
            clock,
            webhook_token,
        }
    }

    async fn import_workout_for_user(
        &self,
        user_id: &str,
        workout: WahooWorkout,
    ) -> Result<ImportedWorkout, WahooWebhookError> {
        let Some(command) = map_workout_to_import_command(user_id, &workout) else {
            return Err(WahooWebhookError::InvalidPayload(
                "Wahoo workout payload did not produce a valid import command".to_string(),
            ));
        };
        let import_outcome = self
            .imports
            .import(command)
            .await
            .map_err(map_import_error)?;
        self.enqueue_fit_enrichment_if_needed(user_id, &workout, &import_outcome)
            .await?;

        Ok(ImportedWorkout {
            completed_workout_id: import_outcome.canonical_entity.entity_id,
            oldest_date: workout.starts.get(..10).map(ToString::to_string),
        })
    }

    async fn enqueue_fit_enrichment_if_needed(
        &self,
        user_id: &str,
        workout: &WahooWorkout,
        import_outcome: &ExternalImportOutcome,
    ) -> Result<(), WahooWebhookError> {
        let has_fit_file = workout
            .workout_summary
            .as_ref()
            .and_then(|summary| summary.file.as_ref())
            .is_some_and(|file| !file.url.trim().is_empty());
        if !has_fit_file {
            return Ok(());
        }

        self.queue
            .enqueue_enrichment(
                user_id,
                &import_outcome.canonical_entity.entity_id,
                workout.id,
            )
            .await
            .map_err(map_queue_error)
    }

    async fn recompute_training_load_if_needed(
        &self,
        user_id: &str,
        oldest_date: Option<&str>,
    ) -> Result<(), WahooWebhookError> {
        let Some(oldest_date) = oldest_date else {
            return Ok(());
        };
        self.training_load
            .recompute_from(user_id, oldest_date, self.clock.now_epoch_seconds())
            .await
            .map_err(map_training_load_error)
    }
}

impl<SettingsRepo, Imports, Wahoo, TrainingLoad, Queue, Time> WahooWebhookUseCases
    for WahooWebhookService<SettingsRepo, Imports, Wahoo, TrainingLoad, Queue, Time>
where
    SettingsRepo: UserSettingsRepository + Clone + 'static,
    Imports: ExternalImportUseCases + Clone + 'static,
    Wahoo: WahooUseCases + Clone + 'static,
    TrainingLoad: TrainingLoadRecomputeUseCases + Clone + 'static,
    Queue: WahooFitEnrichmentQueueUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn import_webhook_workout(
        &self,
        webhook_token: &str,
        wahoo_user_id: i64,
        workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>> {
        let service = self.clone();
        let webhook_token = webhook_token.to_string();
        let expected_webhook_token = self.webhook_token.clone();
        Box::pin(async move {
            let Some(expected_webhook_token) = expected_webhook_token else {
                return Err(WahooWebhookError::NotConfigured);
            };
            if expected_webhook_token != webhook_token {
                return Err(WahooWebhookError::Unauthorized);
            }

            let Some(settings) = service
                .settings_repository
                .find_by_wahoo_user_id(wahoo_user_id)
                .await
                .map_err(map_settings_error)?
            else {
                return Ok(WahooWebhookOutcome::Ignored);
            };

            let imported = service
                .import_workout_for_user(&settings.user_id, workout)
                .await?;
            service
                .recompute_training_load_if_needed(
                    &settings.user_id,
                    imported.oldest_date.as_deref(),
                )
                .await?;

            Ok(WahooWebhookOutcome::Accepted(WahooWebhookAccepted {
                user_id: settings.user_id,
                completed_workout_id: imported.completed_workout_id,
            }))
        })
    }

    fn sync_completed_workouts_for_user(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<ManualWahooSyncResult, WahooWebhookError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut page = 1;
            let mut scanned = 0;
            let mut imported = 0;
            let mut skipped = 0;
            let mut oldest_imported_date = None::<String>;

            loop {
                let list = service
                    .wahoo_service
                    .list_workouts(&user_id, page, DEFAULT_MANUAL_SYNC_PER_PAGE)
                    .await
                    .map_err(map_wahoo_error)?;
                let list_len = list.workouts.len();
                if list_len == 0 {
                    break;
                }

                scanned += list_len;
                for workout in list.workouts {
                    if workout.workout_summary.is_none() {
                        skipped += 1;
                        continue;
                    }

                    let imported_workout =
                        match service.import_workout_for_user(&user_id, workout).await {
                            Ok(imported_workout) => imported_workout,
                            Err(error) => {
                                service
                                    .recompute_training_load_if_needed(
                                        &user_id,
                                        oldest_imported_date.as_deref(),
                                    )
                                    .await?;
                                return Err(error);
                            }
                        };
                    oldest_imported_date =
                        match (oldest_imported_date, imported_workout.oldest_date) {
                            (Some(current), Some(next)) => Some(std::cmp::min(current, next)),
                            (current @ Some(_), None) => current,
                            (None, next) => next,
                        };
                    imported += 1;
                }

                if list_len < DEFAULT_MANUAL_SYNC_PER_PAGE {
                    break;
                }
                page += 1;
            }

            service
                .recompute_training_load_if_needed(&user_id, oldest_imported_date.as_deref())
                .await?;

            Ok(ManualWahooSyncResult {
                scanned,
                imported,
                skipped,
            })
        })
    }
}

fn map_settings_error(error: SettingsError) -> WahooWebhookError {
    match error {
        SettingsError::Unauthenticated => WahooWebhookError::Settings(error.to_string()),
        SettingsError::Repository(message) | SettingsError::Validation(message) => {
            WahooWebhookError::Settings(message)
        }
    }
}

fn map_import_error(error: ExternalImportError) -> WahooWebhookError {
    WahooWebhookError::Import(error.to_string())
}

fn map_queue_error(error: WahooFitEnrichmentError) -> WahooWebhookError {
    WahooWebhookError::Queue(error.to_string())
}

fn map_training_load_error(error: TrainingLoadError) -> WahooWebhookError {
    WahooWebhookError::TrainingLoad(error.to_string())
}

fn map_wahoo_error(error: WahooError) -> WahooWebhookError {
    WahooWebhookError::Wahoo(error.to_string())
}

#[cfg(test)]
mod tests;
