use std::sync::Arc;

use tracing::warn;

use crate::{
    adapters::intervals_icu::import_mapping::{
        map_activity_metrics_to_import_command, map_activity_to_import_command,
    },
    domain::{
        completed_workouts::{
            BackfillCompletedWorkoutDetailsResult, BackfillCompletedWorkoutMetricsResult,
            CompletedWorkoutAdminUseCases, CompletedWorkoutError, CompletedWorkoutRepository,
        },
        external_sync::ExternalImportUseCases,
        identity::Clock,
        intervals::{IntervalsApiPort, IntervalsSettingsPort},
        training_load::TrainingLoadRecomputeUseCases,
    },
};

use super::support::{
    activity_backfills_missing_metrics, earliest_recompute_date, has_backfillable_activity_details,
    needs_detail_backfill, needs_metric_backfill, oldest_workout_date,
};

#[derive(Clone)]
struct MetricsBackfillContext<Repo, Settings, Api, Imports>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
{
    repository: Repo,
    settings: Settings,
    api: Api,
    imports: Imports,
    recompute: Option<Arc<dyn TrainingLoadRecomputeUseCases>>,
    user_id: String,
    oldest: Option<String>,
    newest: Option<String>,
    now_epoch_seconds: i64,
}

struct MetricsBackfillSelection {
    scanned: usize,
    oldest_scanned_date: Option<String>,
    candidates: Vec<crate::domain::completed_workouts::CompletedWorkout>,
}

struct MetricsBackfillProgress {
    enriched: usize,
    skipped: usize,
    failed: usize,
    recomputed_from: Option<String>,
}

#[derive(Clone)]
pub struct IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports, Time>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
    Time: Clock,
{
    repository: Repo,
    settings: Settings,
    api: Api,
    imports: Imports,
    training_load_recompute: Option<Arc<dyn TrainingLoadRecomputeUseCases>>,
    clock: Time,
}

impl<Repo, Settings, Api, Imports, Time>
    IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports, Time>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
    Time: Clock,
{
    pub fn new(
        repository: Repo,
        settings: Settings,
        api: Api,
        imports: Imports,
        clock: Time,
    ) -> Self {
        Self {
            repository,
            settings,
            api,
            imports,
            training_load_recompute: None,
            clock,
        }
    }

    pub fn with_training_load_recompute_service(
        mut self,
        recompute: Arc<dyn TrainingLoadRecomputeUseCases>,
    ) -> Self {
        self.training_load_recompute = Some(recompute);
        self
    }

    fn metrics_context(
        &self,
        user_id: &str,
        oldest: Option<&str>,
        newest: Option<&str>,
    ) -> MetricsBackfillContext<Repo, Settings, Api, Imports> {
        MetricsBackfillContext {
            repository: self.repository.clone(),
            settings: self.settings.clone(),
            api: self.api.clone(),
            imports: self.imports.clone(),
            recompute: self.training_load_recompute.clone(),
            user_id: user_id.to_string(),
            oldest: oldest.map(ToString::to_string),
            newest: newest.map(ToString::to_string),
            now_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }
}

impl<Repo, Settings, Api, Imports> MetricsBackfillContext<Repo, Settings, Api, Imports>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
{
    fn validate_range(&self) -> Result<(), CompletedWorkoutError> {
        if self.oldest.is_some() != self.newest.is_some() {
            return Err(CompletedWorkoutError::Repository(
                "backfill-metrics requires both oldest and newest or neither".to_string(),
            ));
        }

        Ok(())
    }

    async fn select_workouts(&self) -> Result<MetricsBackfillSelection, CompletedWorkoutError> {
        let workouts = match (&self.oldest, &self.newest) {
            (Some(oldest), Some(newest)) => {
                self.repository
                    .list_by_user_id_and_date_range(&self.user_id, oldest, newest)
                    .await?
            }
            (None, None) => self.repository.list_by_user_id(&self.user_id).await?,
            _ => Vec::new(),
        };

        Ok(MetricsBackfillSelection {
            scanned: workouts.len(),
            oldest_scanned_date: oldest_workout_date(&workouts),
            candidates: workouts.into_iter().filter(needs_metric_backfill).collect(),
        })
    }

    async fn recompute_existing_range(
        &self,
        scanned: usize,
        oldest_scanned_date: Option<String>,
    ) -> Result<BackfillCompletedWorkoutMetricsResult, CompletedWorkoutError> {
        if let (Some(recompute), Some(oldest_recompute_date)) =
            (self.recompute.clone(), oldest_scanned_date.clone())
        {
            recompute
                .recompute_from(&self.user_id, &oldest_recompute_date, self.now_epoch_seconds)
                .await
                .map_err(|error| {
                    CompletedWorkoutError::Repository(format!(
                        "metrics backfill found no missing metrics but training load recompute from {oldest_recompute_date} failed: {error}"
                    ))
                })?;
        }

        Ok(BackfillCompletedWorkoutMetricsResult {
            scanned,
            enriched: 0,
            skipped: scanned,
            failed: 0,
            recomputed_from: oldest_scanned_date,
        })
    }

    async fn enrich_metrics(
        &self,
        candidates: &[crate::domain::completed_workouts::CompletedWorkout],
    ) -> Result<MetricsBackfillProgress, CompletedWorkoutError> {
        let credentials = self
            .settings
            .get_credentials(&self.user_id)
            .await
            .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;

        let mut progress = MetricsBackfillProgress {
            enriched: 0,
            skipped: 0,
            failed: 0,
            recomputed_from: None,
        };

        for workout in candidates {
            let Some(source_activity_id) = workout.source_activity_id.as_deref() else {
                progress.skipped += 1;
                continue;
            };

            let detailed_activity = match self
                .api
                .get_activity(&credentials, source_activity_id)
                .await
            {
                Ok(activity) => activity,
                Err(error) => {
                    warn!(
                        user_id = %self.user_id,
                        source_activity_id,
                        error = %error,
                        "metrics backfill failed to fetch activity"
                    );
                    progress.failed += 1;
                    continue;
                }
            };

            if !activity_backfills_missing_metrics(workout, &detailed_activity) {
                progress.skipped += 1;
                continue;
            }

            match self
                .imports
                .import(map_activity_metrics_to_import_command(
                    &self.user_id,
                    workout,
                    &detailed_activity,
                ))
                .await
            {
                Ok(_) => {
                    progress.enriched += 1;
                    if let Some(date) = earliest_recompute_date(workout, &detailed_activity) {
                        progress.recomputed_from = match progress.recomputed_from.take() {
                            Some(current) => Some(std::cmp::min(current, date.to_string())),
                            None => Some(date.to_string()),
                        };
                    }
                }
                Err(error) => {
                    warn!(
                        user_id = %self.user_id,
                        source_activity_id,
                        error = %error,
                        "metrics backfill failed to import activity metrics"
                    );
                    progress.failed += 1;
                }
            }
        }

        Ok(progress)
    }

    async fn recompute_after_enrichment(
        &self,
        progress: &MetricsBackfillProgress,
    ) -> Result<(), CompletedWorkoutError> {
        if let (Some(recompute), Some(oldest_changed)) =
            (self.recompute.clone(), progress.recomputed_from.clone())
        {
            recompute
                .recompute_from(&self.user_id, &oldest_changed, self.now_epoch_seconds)
                .await
                .map_err(|error| {
                    CompletedWorkoutError::Repository(format!(
                        "metrics backfill updated {} workouts but training load recompute from {} failed: {}",
                        progress.enriched, oldest_changed, error
                    ))
                })?;
        }

        Ok(())
    }
}

impl<Repo, Settings, Api, Imports, Time> CompletedWorkoutAdminUseCases
    for IntervalsCompletedWorkoutBackfillService<Repo, Settings, Api, Imports, Time>
where
    Repo: CompletedWorkoutRepository,
    Settings: IntervalsSettingsPort,
    Api: IntervalsApiPort,
    Imports: ExternalImportUseCases,
    Time: Clock,
{
    fn backfill_missing_details(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<BackfillCompletedWorkoutDetailsResult, CompletedWorkoutError>,
    > {
        let repository = self.repository.clone();
        let settings = self.settings.clone();
        let api = self.api.clone();
        let imports = self.imports.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();

        Box::pin(async move {
            let workouts = repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await?;
            let scanned = workouts.len();

            let candidates = workouts
                .into_iter()
                .filter(needs_detail_backfill)
                .collect::<Vec<_>>();

            if candidates.is_empty() {
                return Ok(BackfillCompletedWorkoutDetailsResult {
                    scanned,
                    enriched: 0,
                    skipped: scanned,
                    failed: 0,
                });
            }

            let credentials = settings
                .get_credentials(&user_id)
                .await
                .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?;

            let mut enriched = 0;
            let mut skipped = 0;
            let mut failed = 0;

            for workout in &candidates {
                let Some(source_activity_id) = workout.source_activity_id.as_deref() else {
                    skipped += 1;
                    continue;
                };

                let detailed_activity =
                    match api.get_activity(&credentials, source_activity_id).await {
                        Ok(activity) => activity,
                        Err(_) => {
                            failed += 1;
                            continue;
                        }
                    };

                if !has_backfillable_activity_details(&detailed_activity) {
                    skipped += 1;
                    continue;
                }

                match imports
                    .import(map_activity_to_import_command(&user_id, &detailed_activity))
                    .await
                {
                    Ok(_) => enriched += 1,
                    Err(_) => failed += 1,
                }
            }

            Ok(BackfillCompletedWorkoutDetailsResult {
                scanned,
                enriched,
                skipped: skipped + scanned.saturating_sub(candidates.len()),
                failed,
            })
        })
    }

    fn backfill_missing_metrics(
        &self,
        user_id: &str,
        oldest: Option<&str>,
        newest: Option<&str>,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<BackfillCompletedWorkoutMetricsResult, CompletedWorkoutError>,
    > {
        let context = self.metrics_context(user_id, oldest, newest);

        Box::pin(async move {
            context.validate_range()?;
            let selection = context.select_workouts().await?;
            if selection.candidates.is_empty() {
                return context
                    .recompute_existing_range(selection.scanned, selection.oldest_scanned_date)
                    .await;
            }

            let progress = context.enrich_metrics(&selection.candidates).await?;
            context.recompute_after_enrichment(&progress).await?;
            Ok(BackfillCompletedWorkoutMetricsResult {
                scanned: selection.scanned,
                enriched: progress.enriched,
                skipped: progress.skipped
                    + selection.scanned.saturating_sub(selection.candidates.len()),
                failed: progress.failed,
                recomputed_from: progress.recomputed_from,
            })
        })
    }
}
