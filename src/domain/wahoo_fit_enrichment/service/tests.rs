use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar_view::{
        BoxFuture as CalendarViewBoxFuture, CalendarEntryView, CalendarEntryViewError,
        CalendarEntryViewRefreshPort,
    },
    completed_workouts::{
        BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutDetails,
        CompletedWorkoutError, CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics,
        CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
    },
    identity::Clock,
    training_load::{
        BoxFuture as TrainingLoadBoxFuture, TrainingLoadError, TrainingLoadRecomputeUseCases,
    },
    wahoo::{
        BoxFuture as WahooBoxFuture, WahooAuthExchange, WahooAuthStart, WahooCreatePlan,
        WahooCreateWorkout, WahooError, WahooFileReference, WahooPlan, WahooToken, WahooUpdatePlan,
        WahooUpdateWorkout, WahooUseCases, WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
    },
    wahoo_fit_files::{
        BoxFuture as WahooFitFileBoxFuture, WahooFitFile, WahooFitFileRepository, WahooFitFileStage,
    },
};

use super::*;

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CalendarViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push((user_id, oldest, newest));
            Ok(Vec::new())
        })
    }
}

#[derive(Clone)]
struct FailingCalendarRefresh;

impl CalendarEntryViewRefreshPort for FailingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> CalendarViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        Box::pin(async {
            Err(CalendarEntryViewError::Repository(
                "refresh failed".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl InMemoryCompletedWorkoutRepository {
    fn with_workout(workout: CompletedWorkout) -> Self {
        Self {
            stored: Arc::new(Mutex::new(vec![workout])),
        }
    }

    fn only_workout(&self) -> CompletedWorkout {
        self.stored.lock().unwrap()[0].clone()
    }
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        _user_id: &str,
        _source_activity_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_latest_by_user_id(
        &self,
        _user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> CompletedWorkoutBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.clear();
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryWahooFitFileRepository {
    stored: Arc<Mutex<Vec<WahooFitFile>>>,
}

impl InMemoryWahooFitFileRepository {
    fn only_fit_file(&self) -> WahooFitFile {
        self.stored.lock().unwrap()[0].clone()
    }
}

impl WahooFitFileRepository for InMemoryWahooFitFileRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> WahooFitFileBoxFuture<
        Result<Option<WahooFitFile>, crate::domain::wahoo_fit_files::WahooFitFileError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|fit_file| {
                (fit_file.user_id == user_id
                    && fit_file.completed_workout_id == completed_workout_id)
                    .then(|| fit_file.clone())
            }))
        })
    }

    fn upsert(
        &self,
        fit_file: WahooFitFile,
    ) -> WahooFitFileBoxFuture<
        Result<WahooFitFile, crate::domain::wahoo_fit_files::WahooFitFileError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == fit_file.user_id
                    && existing.completed_workout_id == fit_file.completed_workout_id)
            });
            stored.push(fit_file.clone());
            Ok(fit_file)
        })
    }
}

#[derive(Clone, Default)]
struct MissingSecondReadWahooFitFileRepository {
    stored: Arc<Mutex<Vec<WahooFitFile>>>,
    find_calls: Arc<Mutex<usize>>,
}

impl MissingSecondReadWahooFitFileRepository {
    fn only_fit_file(&self) -> WahooFitFile {
        self.stored.lock().unwrap()[0].clone()
    }
}

impl WahooFitFileRepository for MissingSecondReadWahooFitFileRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> WahooFitFileBoxFuture<
        Result<Option<WahooFitFile>, crate::domain::wahoo_fit_files::WahooFitFileError>,
    > {
        let stored = self.stored.clone();
        let find_calls = self.find_calls.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let mut find_calls = find_calls.lock().unwrap();
            *find_calls += 1;
            if *find_calls > 1 {
                return Ok(None);
            }
            Ok(stored.lock().unwrap().iter().find_map(|fit_file| {
                (fit_file.user_id == user_id
                    && fit_file.completed_workout_id == completed_workout_id)
                    .then(|| fit_file.clone())
            }))
        })
    }

    fn upsert(
        &self,
        fit_file: WahooFitFile,
    ) -> WahooFitFileBoxFuture<
        Result<WahooFitFile, crate::domain::wahoo_fit_files::WahooFitFileError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == fit_file.user_id
                    && existing.completed_workout_id == fit_file.completed_workout_id)
            });
            stored.push(fit_file.clone());
            Ok(fit_file)
        })
    }
}

#[derive(Clone)]
struct FakeWahooService {
    summary: Option<WahooWorkoutSummary>,
    file_bytes: Vec<u8>,
    download_calls: Arc<Mutex<usize>>,
}

impl FakeWahooService {
    fn with_file(file_url: &str, file_bytes: Vec<u8>) -> Self {
        Self {
            summary: Some(WahooWorkoutSummary {
                id: 42,
                name: Some("Morning Ride".to_string()),
                ascent_meters: None,
                cadence_avg_rpm: None,
                calories: None,
                distance_meters: None,
                duration_active_seconds: None,
                duration_paused_seconds: None,
                duration_total_seconds: None,
                heart_rate_avg_bpm: None,
                normalized_power_watts: None,
                training_stress_score: None,
                average_power_watts: None,
                speed_avg_mps: None,
                total_work_joules: None,
                time_zone: None,
                manual: false,
                edited: false,
                fitness_app_id: None,
                file: Some(WahooFileReference {
                    url: file_url.to_string(),
                }),
                created_at: None,
                updated_at: None,
            }),
            file_bytes,
            download_calls: Arc::new(Mutex::new(0)),
        }
    }

    fn download_calls(&self) -> usize {
        *self.download_calls.lock().unwrap()
    }
}

impl WahooUseCases for FakeWahooService {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> WahooBoxFuture<Result<WahooAuthStart, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> WahooBoxFuture<Result<WahooAuthExchange, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn ensure_token(&self, _user_id: &str) -> WahooBoxFuture<Result<WahooToken, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        _page: usize,
        _per_page: usize,
    ) -> WahooBoxFuture<Result<WahooWorkoutList, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        let summary = self.summary.clone();
        Box::pin(async move { Ok(summary) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> WahooBoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: WahooCreatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        _plan_id: i64,
        _request: WahooUpdatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: WahooCreateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
        _request: WahooUpdateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> WahooBoxFuture<Result<Vec<u8>, WahooError>> {
        let file_bytes = self.file_bytes.clone();
        let download_calls = self.download_calls.clone();
        Box::pin(async move {
            *download_calls.lock().unwrap() += 1;
            Ok(file_bytes)
        })
    }
}

#[derive(Clone)]
struct FakeParser {
    responses: Arc<Mutex<Vec<Result<ParsedWahooFitWorkout, String>>>>,
}

impl FakeParser {
    fn with_responses(responses: Vec<Result<ParsedWahooFitWorkout, String>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }
}

impl WahooFitParserPort for FakeParser {
    fn parse_fit_workout(
        &self,
        _file_bytes: &[u8],
    ) -> BoxFuture<Result<ParsedWahooFitWorkout, String>> {
        let responses = self.responses.clone();
        Box::pin(async move { responses.lock().unwrap().remove(0) })
    }
}

#[derive(Clone, Default)]
struct RecordingTrainingLoadRecomputeService {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingTrainingLoadRecomputeService {
    fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TrainingLoadRecomputeUseCases for RecordingTrainingLoadRecomputeService {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> TrainingLoadBoxFuture<Result<(), TrainingLoadError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest_date, now_epoch_seconds));
            Ok(())
        })
    }
}

fn sample_workout() -> CompletedWorkout {
    CompletedWorkout::new(
        "wahoo-workout:42".to_string(),
        "user-1".to_string(),
        "2026-05-01T08:00:00Z".to_string(),
        Some("42".to_string()),
        None,
        Some("Morning Ride".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        true,
        Some(1200),
        Some(10000.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(45),
            normalized_power_watts: Some(210),
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(190),
            ftp_watts: None,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        Some(
            "Detailed Wahoo workout data is still being processed. Please check back soon."
                .to_string(),
        ),
    )
}

fn parsed_workout() -> ParsedWahooFitWorkout {
    ParsedWahooFitWorkout {
        duration_seconds: Some(1800),
        distance_meters: Some(25000.0),
        activity_type: Some("Ride".to_string()),
        trainer: Some(false),
        metrics: CompletedWorkoutMetrics {
            training_stress_score: Some(62),
            normalized_power_watts: Some(225),
            intensity_factor: Some(0.85),
            efficiency_factor: None,
            variability_index: Some(1.03),
            average_power_watts: Some(218),
            ftp_watts: None,
            total_work_joules: Some(750),
            calories: Some(650),
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![200, 220, 240])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    }
}

#[tokio::test]
async fn enrich_completed_workout_updates_workout_and_fit_file() {
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(sample_workout());
    let fit_files = InMemoryWahooFitFileRepository::default();
    let recompute = Arc::new(RecordingTrainingLoadRecomputeService::default());
    let service = WahooFitEnrichmentService::new(
        Arc::new(FakeWahooService::with_file(
            "https://example.test/workout.fit",
            vec![1, 2, 3, 4],
        )),
        workouts.clone(),
        fit_files.clone(),
        FakeParser::with_responses(vec![Ok(parsed_workout())]),
        FixedClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect("fit enrichment should succeed");

    let workout = workouts.only_workout();
    assert_eq!(workout.details_unavailable_reason, None);
    assert_eq!(workout.duration_seconds, Some(1800));
    assert_eq!(workout.distance_meters, Some(25000.0));
    assert_eq!(workout.metrics.training_stress_score, Some(62));
    assert_eq!(workout.details.streams.len(), 1);

    let fit_file = fit_files.only_fit_file();
    assert_eq!(fit_file.stage, WahooFitFileStage::Enriched);
    assert_eq!(
        fit_file.file_url.as_deref(),
        Some("https://example.test/workout.fit")
    );
    assert_eq!(fit_file.raw_fit_bytes, Some(vec![1, 2, 3, 4]));
    assert_eq!(recompute.calls().len(), 1);
}

#[tokio::test]
async fn enrich_completed_workout_refreshes_calendar_day() {
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(sample_workout());
    let fit_files = InMemoryWahooFitFileRepository::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = WahooFitEnrichmentService::new(
        Arc::new(FakeWahooService::with_file(
            "https://example.test/workout.fit",
            vec![1, 2, 3, 4],
        )),
        workouts,
        fit_files,
        FakeParser::with_responses(vec![Ok(parsed_workout())]),
        FixedClock,
    )
    .with_calendar_view_refresh(refresh.clone());

    service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect("fit enrichment should succeed");

    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-01".to_string(),
            "2026-05-01".to_string(),
        )]
    );
}

#[tokio::test]
async fn enrich_completed_workout_returns_retryable_error_when_calendar_refresh_fails() {
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(sample_workout());
    let fit_files = InMemoryWahooFitFileRepository::default();
    let service = WahooFitEnrichmentService::new(
        Arc::new(FakeWahooService::with_file(
            "https://example.test/workout.fit",
            vec![1, 2, 3, 4],
        )),
        workouts.clone(),
        fit_files.clone(),
        FakeParser::with_responses(vec![Ok(parsed_workout())]),
        FixedClock,
    )
    .with_calendar_view_refresh(FailingCalendarRefresh);

    let error = service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect_err("fit enrichment should fail when refresh fails");

    assert!(matches!(
        error,
        WahooFitEnrichmentError::CalendarViewRefresh(_)
    ));
    assert_eq!(workouts.only_workout().details_unavailable_reason, None);
    assert_eq!(fit_files.only_fit_file().stage, WahooFitFileStage::Parsed);
}

#[tokio::test]
async fn parse_failure_keeps_stored_fit_bytes_for_retry() {
    let wahoo = Arc::new(FakeWahooService::with_file(
        "https://example.test/workout.fit",
        vec![9, 8, 7, 6],
    ));
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(sample_workout());
    let fit_files = InMemoryWahooFitFileRepository::default();
    let parser = FakeParser::with_responses(vec![
        Err("parse exploded".to_string()),
        Ok(parsed_workout()),
    ]);
    let service = WahooFitEnrichmentService::new(
        wahoo.clone(),
        workouts.clone(),
        fit_files.clone(),
        parser,
        FixedClock,
    );

    let first_error = service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect_err("first parse should fail");
    assert!(matches!(first_error, WahooFitEnrichmentError::Parse(_)));
    assert_eq!(
        fit_files.only_fit_file().raw_fit_bytes,
        Some(vec![9, 8, 7, 6])
    );
    assert_eq!(fit_files.only_fit_file().stage, WahooFitFileStage::Stored);
    assert_eq!(wahoo.download_calls(), 1);

    service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect("retry should reuse stored bytes and succeed");

    assert_eq!(wahoo.download_calls(), 1);
    assert_eq!(fit_files.only_fit_file().stage, WahooFitFileStage::Enriched);
    assert_eq!(workouts.only_workout().details_unavailable_reason, None);
}

#[tokio::test]
async fn enrich_completed_workout_does_not_require_second_fit_file_read() {
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(sample_workout());
    let fit_files = MissingSecondReadWahooFitFileRepository::default();
    let service = WahooFitEnrichmentService::new(
        Arc::new(FakeWahooService::with_file(
            "https://example.test/workout.fit",
            vec![1, 2, 3, 4],
        )),
        workouts,
        fit_files.clone(),
        FakeParser::with_responses(vec![Ok(parsed_workout())]),
        FixedClock,
    );

    service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect("fit enrichment should succeed without re-reading the fit file");

    let fit_file = fit_files.only_fit_file();
    assert_eq!(fit_file.stage, WahooFitFileStage::Enriched);
    assert_eq!(fit_file.raw_fit_bytes, Some(vec![1, 2, 3, 4]));
}

#[tokio::test]
async fn enrich_completed_workout_preserves_existing_non_empty_detail_sections() {
    let mut workout = sample_workout();
    workout.details.interval_groups = vec![CompletedWorkoutIntervalGroup {
        id: "group-1".to_string(),
        count: Some(1),
        start_index: Some(0),
        elapsed_time_seconds: Some(600),
        moving_time_seconds: Some(600),
        distance_meters: Some(3000.0),
        average_power_watts: Some(180),
        normalized_power_watts: Some(185),
        training_stress_score: Some(12.5),
        average_heart_rate_bpm: Some(135),
        average_cadence_rpm: Some(90.0),
        average_speed_mps: Some(5.0),
        average_stride_meters: None,
    }];
    let workouts = InMemoryCompletedWorkoutRepository::with_workout(workout);
    let fit_files = InMemoryWahooFitFileRepository::default();
    let service = WahooFitEnrichmentService::new(
        Arc::new(FakeWahooService::with_file(
            "https://example.test/workout.fit",
            vec![1, 2, 3, 4],
        )),
        workouts.clone(),
        fit_files,
        FakeParser::with_responses(vec![Ok(parsed_workout())]),
        FixedClock,
    );

    service
        .enrich_completed_workout("user-1", "wahoo-workout:42", 42)
        .await
        .expect("fit enrichment should preserve existing richer detail sections");

    let stored = workouts.only_workout();
    assert_eq!(stored.details.streams.len(), 1);
    assert_eq!(stored.details.interval_groups.len(), 1);
    assert_eq!(stored.details.interval_groups[0].id, "group-1");
    assert_eq!(
        stored.details.interval_groups[0].training_stress_score,
        Some(12.5)
    );
}
