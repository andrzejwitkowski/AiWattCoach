use sha2::{Digest, Sha256};
use std::{future::Future, pin::Pin, sync::Arc};

use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutError, CompletedWorkoutMetrics,
        CompletedWorkoutRepository,
    },
    identity::Clock,
    training_load::TrainingLoadRecomputeUseCases,
    wahoo::WahooUseCases,
    wahoo_fit_files::{WahooFitFile, WahooFitFileError, WahooFitFileRepository},
};

use super::refresh::refresh_completed_workout_day;
use super::{ParsedWahooFitWorkout, WahooFitEnrichmentError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait WahooFitParserPort: Clone + Send + Sync + 'static {
    fn parse_fit_workout(
        &self,
        file_bytes: &[u8],
    ) -> BoxFuture<Result<ParsedWahooFitWorkout, String>>;
}

#[derive(Clone)]
pub struct WahooFitEnrichmentService<
    Wahoo,
    Workouts,
    FitFiles,
    Parser,
    Time,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Wahoo: WahooUseCases + ?Sized + 'static,
    Workouts: CompletedWorkoutRepository,
    FitFiles: WahooFitFileRepository,
    Parser: WahooFitParserPort,
    Time: Clock + 'static,
    Refresh: CalendarEntryViewRefreshPort,
{
    wahoo: Arc<Wahoo>,
    completed_workouts: Workouts,
    fit_files: FitFiles,
    parser: Parser,
    clock: Time,
    refresh: Refresh,
    training_load_recompute: Option<Arc<dyn TrainingLoadRecomputeUseCases>>,
}

impl<Wahoo, Workouts, FitFiles, Parser, Time>
    WahooFitEnrichmentService<Wahoo, Workouts, FitFiles, Parser, Time, NoopCalendarEntryViewRefresh>
where
    Wahoo: WahooUseCases + ?Sized + 'static,
    Workouts: CompletedWorkoutRepository,
    FitFiles: WahooFitFileRepository,
    Parser: WahooFitParserPort,
    Time: Clock + 'static,
{
    pub fn new(
        wahoo: Arc<Wahoo>,
        completed_workouts: Workouts,
        fit_files: FitFiles,
        parser: Parser,
        clock: Time,
    ) -> Self {
        Self {
            wahoo,
            completed_workouts,
            fit_files,
            parser,
            clock,
            refresh: NoopCalendarEntryViewRefresh,
            training_load_recompute: None,
        }
    }
}

impl<Wahoo, Workouts, FitFiles, Parser, Time, Refresh>
    WahooFitEnrichmentService<Wahoo, Workouts, FitFiles, Parser, Time, Refresh>
where
    Wahoo: WahooUseCases + ?Sized + 'static,
    Workouts: CompletedWorkoutRepository,
    FitFiles: WahooFitFileRepository,
    Parser: WahooFitParserPort,
    Time: Clock + 'static,
    Refresh: CalendarEntryViewRefreshPort,
{
    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> WahooFitEnrichmentService<Wahoo, Workouts, FitFiles, Parser, Time, NewRefresh>
    where
        NewRefresh: CalendarEntryViewRefreshPort,
    {
        WahooFitEnrichmentService {
            wahoo: self.wahoo,
            completed_workouts: self.completed_workouts,
            fit_files: self.fit_files,
            parser: self.parser,
            clock: self.clock,
            refresh,
            training_load_recompute: self.training_load_recompute,
        }
    }

    pub fn with_training_load_recompute_service(
        mut self,
        training_load_recompute: Arc<dyn TrainingLoadRecomputeUseCases>,
    ) -> Self {
        self.training_load_recompute = Some(training_load_recompute);
        self
    }

    pub fn enrich_completed_workout(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> BoxFuture<Result<(), WahooFitEnrichmentError>> {
        let wahoo = self.wahoo.clone();
        let completed_workouts = self.completed_workouts.clone();
        let fit_files = self.fit_files.clone();
        let parser = self.parser.clone();
        let clock = self.clock.clone();
        let refresh = self.refresh.clone();
        let training_load_recompute = self.training_load_recompute.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let service = WahooFitEnrichmentService {
                wahoo,
                completed_workouts,
                fit_files,
                parser,
                clock,
                refresh,
                training_load_recompute,
            };
            let workout = service
                .load_completed_workout(&user_id, &completed_workout_id)
                .await?;
            let fit_file = service
                .load_or_create_fit_file(&user_id, &completed_workout_id, wahoo_workout_id)
                .await?;
            let (fit_file, file_bytes) = service
                .ensure_fit_file_bytes(&user_id, wahoo_workout_id, fit_file)
                .await?;
            let parsed = service.parse_fit_workout(&file_bytes).await?;
            let fit_file = service.persist_parsed_fit_file(fit_file).await?;
            let enriched_workout = merge_workout_enrichment(workout, parsed);
            service
                .persist_enriched_workout(enriched_workout.clone())
                .await?;
            service
                .refresh_completed_workout_day(&enriched_workout)
                .await?;
            service
                .recompute_training_load_if_needed(&enriched_workout)
                .await?;
            service.persist_enriched_fit_file(fit_file).await?;
            Ok(())
        })
    }

    async fn load_completed_workout(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> Result<CompletedWorkout, WahooFitEnrichmentError> {
        self.completed_workouts
            .find_by_user_id_and_completed_workout_id(user_id, completed_workout_id)
            .await
            .map_err(map_completed_workout_error)?
            .ok_or(WahooFitEnrichmentError::NotFound)
    }

    async fn load_or_create_fit_file(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> Result<WahooFitFile, WahooFitEnrichmentError> {
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Ok(self
            .fit_files
            .find_by_user_id_and_completed_workout_id(user_id, completed_workout_id)
            .await
            .map_err(map_fit_file_error)?
            .unwrap_or_else(|| {
                WahooFitFile::new(
                    user_id.to_string(),
                    completed_workout_id.to_string(),
                    wahoo_workout_id,
                    now_epoch_seconds,
                )
            }))
    }

    async fn ensure_fit_file_bytes(
        &self,
        user_id: &str,
        wahoo_workout_id: i64,
        fit_file: WahooFitFile,
    ) -> Result<(WahooFitFile, Vec<u8>), WahooFitEnrichmentError> {
        if let Some(raw_fit_bytes) = fit_file.raw_fit_bytes.clone() {
            return Ok((fit_file, raw_fit_bytes));
        }

        let file_url = self.load_file_url(user_id, wahoo_workout_id).await?;

        let raw_fit_bytes = self
            .wahoo
            .download_workout_file(&file_url)
            .await
            .map_err(WahooFitEnrichmentError::Wahoo)?;
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        let stored_fit_file = fit_file.mark_stored(
            file_url,
            sha256_hex(&raw_fit_bytes),
            raw_fit_bytes.clone(),
            now_epoch_seconds,
        );
        let stored_fit_file = self
            .fit_files
            .upsert(stored_fit_file)
            .await
            .map_err(map_fit_file_error)?;
        Ok((stored_fit_file, raw_fit_bytes))
    }

    async fn load_file_url(
        &self,
        user_id: &str,
        wahoo_workout_id: i64,
    ) -> Result<String, WahooFitEnrichmentError> {
        let summary = self
            .wahoo
            .get_workout_summary(user_id, wahoo_workout_id)
            .await
            .map_err(WahooFitEnrichmentError::Wahoo)?;
        summary
            .and_then(|summary| summary.file.map(|file| file.url))
            .filter(|url| !url.trim().is_empty())
            .ok_or_else(|| {
                WahooFitEnrichmentError::DownloadUnavailable(format!(
                    "Wahoo workout {wahoo_workout_id} does not expose a FIT file URL"
                ))
            })
    }

    async fn parse_fit_workout(
        &self,
        file_bytes: &[u8],
    ) -> Result<ParsedWahooFitWorkout, WahooFitEnrichmentError> {
        let parsed = self
            .parser
            .parse_fit_workout(file_bytes)
            .await
            .map_err(WahooFitEnrichmentError::Parse)?;
        if !has_any_details(&parsed.details) {
            return Err(WahooFitEnrichmentError::Parse(
                "FIT file did not contain usable workout details".to_string(),
            ));
        }
        Ok(parsed)
    }

    async fn persist_parsed_fit_file(
        &self,
        fit_file: WahooFitFile,
    ) -> Result<WahooFitFile, WahooFitEnrichmentError> {
        self.fit_files
            .upsert(fit_file.mark_parsed(self.clock.now_epoch_seconds()))
            .await
            .map_err(map_fit_file_error)
    }

    async fn persist_enriched_workout(
        &self,
        workout: CompletedWorkout,
    ) -> Result<CompletedWorkout, WahooFitEnrichmentError> {
        self.completed_workouts
            .upsert(workout)
            .await
            .map_err(map_completed_workout_error)
    }

    async fn recompute_training_load_if_needed(
        &self,
        workout: &CompletedWorkout,
    ) -> Result<(), WahooFitEnrichmentError> {
        let Some(training_load_recompute) = self.training_load_recompute.clone() else {
            return Ok(());
        };
        let Some(oldest_date) = workout.start_date_local.get(..10) else {
            return Ok(());
        };
        training_load_recompute
            .recompute_from(
                &workout.user_id,
                oldest_date,
                self.clock.now_epoch_seconds(),
            )
            .await
            .map_err(|error| WahooFitEnrichmentError::TrainingLoad(error.to_string()))
    }

    async fn persist_enriched_fit_file(
        &self,
        fit_file: WahooFitFile,
    ) -> Result<WahooFitFile, WahooFitEnrichmentError> {
        self.fit_files
            .upsert(fit_file.mark_enriched(self.clock.now_epoch_seconds()))
            .await
            .map_err(map_fit_file_error)
    }

    async fn refresh_completed_workout_day(
        &self,
        workout: &CompletedWorkout,
    ) -> Result<(), WahooFitEnrichmentError> {
        refresh_completed_workout_day(&self.refresh, &workout.user_id, &workout.start_date_local)
            .await
    }
}

fn map_completed_workout_error(error: CompletedWorkoutError) -> WahooFitEnrichmentError {
    WahooFitEnrichmentError::CompletedWorkoutRepository(error.to_string())
}

fn map_fit_file_error(error: WahooFitFileError) -> WahooFitEnrichmentError {
    WahooFitEnrichmentError::FitFileRepository(error.to_string())
}

fn sha256_hex(file_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_bytes);
    format!("{:x}", hasher.finalize())
}

fn has_any_details(details: &CompletedWorkoutDetails) -> bool {
    !details.intervals.is_empty()
        || !details.interval_groups.is_empty()
        || !details.streams.is_empty()
        || !details.interval_summary.is_empty()
        || !details.skyline_chart.is_empty()
        || !details.power_zone_times.is_empty()
        || !details.heart_rate_zone_times.is_empty()
        || !details.pace_zone_times.is_empty()
        || !details.gap_zone_times.is_empty()
}

fn merge_workout_enrichment(
    existing: CompletedWorkout,
    parsed: ParsedWahooFitWorkout,
) -> CompletedWorkout {
    let merged_details = merge_workout_details(existing.details.clone(), parsed.details);
    CompletedWorkout::new(
        existing.completed_workout_id,
        existing.user_id,
        existing.start_date_local,
        existing.source_activity_id,
        existing.planned_workout_id,
        existing.name,
        existing.description,
        parsed.activity_type.or(existing.activity_type),
        existing.external_id,
        parsed.trainer.unwrap_or(existing.trainer),
        parsed.duration_seconds.or(existing.duration_seconds),
        parsed.distance_meters.or(existing.distance_meters),
        merge_metrics(existing.metrics, parsed.metrics),
        merged_details,
        None,
    )
}

fn merge_metrics(
    existing: CompletedWorkoutMetrics,
    parsed: CompletedWorkoutMetrics,
) -> CompletedWorkoutMetrics {
    CompletedWorkoutMetrics {
        training_stress_score: parsed
            .training_stress_score
            .or(existing.training_stress_score),
        normalized_power_watts: parsed
            .normalized_power_watts
            .or(existing.normalized_power_watts),
        intensity_factor: parsed.intensity_factor.or(existing.intensity_factor),
        efficiency_factor: parsed.efficiency_factor.or(existing.efficiency_factor),
        variability_index: parsed.variability_index.or(existing.variability_index),
        average_power_watts: parsed.average_power_watts.or(existing.average_power_watts),
        ftp_watts: parsed.ftp_watts.or(existing.ftp_watts),
        total_work_joules: parsed.total_work_joules.or(existing.total_work_joules),
        calories: parsed.calories.or(existing.calories),
        trimp: parsed.trimp.or(existing.trimp),
        power_load: parsed.power_load.or(existing.power_load),
        heart_rate_load: parsed.heart_rate_load.or(existing.heart_rate_load),
        pace_load: parsed.pace_load.or(existing.pace_load),
        strain_score: parsed.strain_score.or(existing.strain_score),
    }
}

fn merge_workout_details(
    existing: CompletedWorkoutDetails,
    parsed: CompletedWorkoutDetails,
) -> CompletedWorkoutDetails {
    CompletedWorkoutDetails {
        intervals: prefer_non_empty(parsed.intervals, existing.intervals),
        interval_groups: prefer_non_empty(parsed.interval_groups, existing.interval_groups),
        streams: prefer_non_empty(parsed.streams, existing.streams),
        interval_summary: prefer_non_empty(parsed.interval_summary, existing.interval_summary),
        skyline_chart: prefer_non_empty(parsed.skyline_chart, existing.skyline_chart),
        power_zone_times: prefer_non_empty(parsed.power_zone_times, existing.power_zone_times),
        heart_rate_zone_times: prefer_non_empty(
            parsed.heart_rate_zone_times,
            existing.heart_rate_zone_times,
        ),
        pace_zone_times: prefer_non_empty(parsed.pace_zone_times, existing.pace_zone_times),
        gap_zone_times: prefer_non_empty(parsed.gap_zone_times, existing.gap_zone_times),
    }
}

fn prefer_non_empty<T>(incoming: Vec<T>, existing: Vec<T>) -> Vec<T> {
    if incoming.is_empty() {
        existing
    } else {
        incoming
    }
}
#[cfg(test)]
mod tests;
