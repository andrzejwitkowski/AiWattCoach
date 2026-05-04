use std::{
    fs,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        calendar::{CalendarService, HiddenCalendarEventSource},
        calendar_labels::{CalendarLabelSource, CalendarLabelsService},
        calendar_view::{
            BoxFuture as CalendarBoxFuture, CalendarEntryKind, CalendarEntrySync,
            CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRepository,
            ManualCalendarRefreshResult, ManualCalendarRefreshUseCases,
        },
        completed_workouts::{
            CompletedWorkout, CompletedWorkoutError, CompletedWorkoutReadService,
            CompletedWorkoutRepository,
        },
        external_sync::NoopExternalSyncStateRepository,
        identity::{Clock, IdentityUseCases},
        intervals::{DateRange, IntervalsUseCases},
        races::RaceUseCases,
        training_plan::{
            BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
            TrainingPlanProjectionRepository, TrainingPlanReplacementResult, TrainingPlanSnapshot,
        },
        workout_summary::{
            BoxFuture as WorkoutSummaryBoxFuture, ConversationMessage, SaveSummaryResult,
            SendMessageResult, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
            WorkoutSummaryUseCases,
        },
    },
    Settings,
};
use mongodb::Client;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static SHARED_FRONTEND_FIXTURE: OnceLock<FrontendFixture> = OnceLock::new();

pub(crate) async fn intervals_test_app(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_and_calendar_entries(
        identity_service,
        intervals_service,
        EmptyTrainingPlanProjectionRepository,
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        TestWorkoutSummaryService::default(),
    )
    .await
}

pub(crate) async fn intervals_test_app_with_projections(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_and_calendar_entries(
        identity_service,
        intervals_service,
        projections,
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        TestWorkoutSummaryService::default(),
    )
    .await
}

pub(crate) async fn intervals_test_app_with_calendar_entries(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_calendar_entries_and_completed_workouts(
        identity_service,
        intervals_service,
        EmptyTrainingPlanProjectionRepository,
        calendar_entry_views,
        InMemoryCompletedWorkoutRepository::default(),
        TestWorkoutSummaryService::default(),
    )
    .await
}

pub(crate) async fn intervals_test_app_with_manual_calendar_refresh_service(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    manual_calendar_refresh_service: Arc<dyn ManualCalendarRefreshUseCases>,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();
    let completed_workout_repository = InMemoryCompletedWorkoutRepository::default();
    let calendar_service = Arc::new(
        CalendarService::new(
            intervals_service.clone(),
            InMemoryCalendarEntryViewRepository::default(),
            EmptyTrainingPlanProjectionRepository,
            NoopExternalSyncStateRepository,
            TestClock,
        )
        .with_completed_workouts(completed_workout_repository.clone()),
    );
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(EmptyCalendarLabelSource));
    let completed_workout_service = Arc::new(CompletedWorkoutReadService::new(
        completed_workout_repository,
    ));

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_calendar_service(calendar_service)
        .with_calendar_labels_service(calendar_labels_service)
        .with_manual_calendar_refresh_service(manual_calendar_refresh_service)
        .with_completed_workout_service(completed_workout_service)
        .with_intervals_service(Arc::new(intervals_service)),
        fixture.dist_dir(),
    )
}

pub(crate) async fn intervals_test_app_with_calendar_entries_and_completed_workouts(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
) -> axum::Router {
    intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
        identity_service,
        intervals_service,
        calendar_entry_views,
        completed_workouts,
        TestWorkoutSummaryService::default(),
    )
    .await
}

pub(crate) async fn intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
    workout_summary_service: impl WorkoutSummaryUseCases + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_calendar_entries_and_completed_workouts(
        identity_service,
        intervals_service,
        EmptyTrainingPlanProjectionRepository,
        calendar_entry_views,
        completed_workouts,
        workout_summary_service,
    )
    .await
}

async fn intervals_test_app_with_projections_calendar_entries_and_completed_workouts(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
    workout_summary_service: impl WorkoutSummaryUseCases + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_and_calendar_entries(
        identity_service,
        intervals_service,
        projections,
        calendar_entry_views,
        completed_workouts,
        workout_summary_service,
    )
    .await
}

pub(crate) async fn intervals_test_app_with_projections_and_calendar_entries(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
    workout_summary_service: impl WorkoutSummaryUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();
    let completed_workout_repository = completed_workouts;
    let calendar_service = Arc::new(
        CalendarService::new(
            intervals_service.clone(),
            calendar_entry_views,
            projections,
            NoopExternalSyncStateRepository,
            TestClock,
        )
        .with_completed_workouts(completed_workout_repository.clone()),
    );
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(EmptyCalendarLabelSource));
    let manual_calendar_refresh_service = Arc::new(TestManualCalendarRefreshService);
    let completed_workout_service = Arc::new(CompletedWorkoutReadService::new(
        completed_workout_repository,
    ));

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_calendar_service(calendar_service)
        .with_calendar_labels_service(calendar_labels_service)
        .with_manual_calendar_refresh_service(manual_calendar_refresh_service)
        .with_completed_workout_service(completed_workout_service)
        .with_workout_summary_service(Arc::new(workout_summary_service))
        .with_intervals_service(Arc::new(intervals_service)),
        fixture.dist_dir(),
    )
}

pub(crate) async fn intervals_test_app_with_all_services(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_label_source: impl CalendarLabelSource + Clone + 'static,
    _hidden_calendar_event_source: impl HiddenCalendarEventSource + Clone + 'static,
    race_service: impl RaceUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();
    let calendar_service = Arc::new(
        CalendarService::new(
            intervals_service.clone(),
            InMemoryCalendarEntryViewRepository::default(),
            projections,
            NoopExternalSyncStateRepository,
            TestClock,
        )
        .with_completed_workouts(InMemoryCompletedWorkoutRepository::default()),
    );
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(calendar_label_source));
    let manual_calendar_refresh_service = Arc::new(TestManualCalendarRefreshService);

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_calendar_service(calendar_service)
        .with_calendar_labels_service(calendar_labels_service)
        .with_manual_calendar_refresh_service(manual_calendar_refresh_service)
        .with_race_service(Arc::new(race_service))
        .with_intervals_service(Arc::new(intervals_service)),
        fixture.dist_dir(),
    )
}

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(crate) struct EmptyTrainingPlanProjectionRepository;

impl TrainingPlanProjectionRepository for EmptyTrainingPlanProjectionRepository {
    fn list_active_by_user_id(
        &self,
        _user_id: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_active_by_operation_key(
        &self,
        _operation_key: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        _user_id: &str,
        _operation_key: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> TrainingPlanBoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>> {
        Box::pin(async move {
            Ok(TrainingPlanReplacementResult {
                snapshot,
                projected_days,
                superseded_date_range: None,
            })
        })
    }
}

#[derive(Clone, Default)]
struct EmptyCalendarLabelSource;

#[derive(Clone, Default)]
pub(crate) struct InMemoryCompletedWorkoutRepository {
    stored: Arc<std::sync::Mutex<Vec<CompletedWorkout>>>,
}

impl InMemoryCompletedWorkoutRepository {
    pub(crate) fn with_workouts(workouts: Vec<CompletedWorkout>) -> Self {
        Self {
            stored: Arc::new(std::sync::Mutex::new(workouts)),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    stored: Arc<std::sync::Mutex<Vec<WorkoutSummary>>>,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            stored: Arc::new(std::sync::Mutex::new(summaries)),
        }
    }
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            stored
                .lock()
                .unwrap()
                .iter()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
                .cloned()
                .ok_or(WorkoutSummaryError::NotFound)
        })
    }

    fn create_summary(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn list_summaries(
        &self,
        _user_id: &str,
        _workout_ids: Vec<String>,
    ) -> WorkoutSummaryBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn mark_saved(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn reopen_summary(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: WorkoutRecap,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn send_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _content: String,
    ) -> WorkoutSummaryBoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn append_user_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _content: String,
    ) -> WorkoutSummaryBoxFuture<
        Result<aiwattcoach::domain::workout_summary::PersistedUserMessage, WorkoutSummaryError>,
    > {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }

    fn generate_coach_reply(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _user_message_id: String,
    ) -> WorkoutSummaryBoxFuture<
        Result<aiwattcoach::domain::workout_summary::CoachReply, WorkoutSummaryError>,
    > {
        Box::pin(async {
            Err(WorkoutSummaryError::Repository(
                "not implemented in test".to_string(),
            ))
        })
    }
}

pub(crate) fn sample_workout_summary(user_id: &str, workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::<ConversationMessage>::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: Some(1_700_000_100),
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_100,
    }
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
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
        user_id: &str,
        source_activity_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

impl CalendarLabelSource for EmptyCalendarLabelSource {
    fn list_labels(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar_labels::BoxFuture<
        Result<
            Vec<aiwattcoach::domain::calendar_labels::CalendarLabel>,
            aiwattcoach::domain::calendar_labels::CalendarLabelError,
        >,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryCalendarEntryViewRepository {
    stored: Arc<std::sync::Mutex<Vec<CalendarEntryView>>>,
}

impl CalendarEntryViewRepository for InMemoryCalendarEntryViewRepository {
    fn find_oldest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> CalendarBoxFuture<Result<Option<String>, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .map(|entry| entry.date.clone())
                .min())
        })
    }

    fn find_newest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> CalendarBoxFuture<Result<Option<String>, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .map(|entry| entry.date.clone())
                .max())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .filter(|entry| entry.date >= oldest && entry.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        entry: CalendarEntryView,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<CalendarEntryView, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == entry.user_id && existing.entry_id == entry.entry_id)
            });
            stored.push(entry.clone());
            Ok(entry)
        })
    }

    fn replace_all_for_user(
        &self,
        user_id: &str,
        entries: Vec<CalendarEntryView>,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| existing.user_id != user_id);
            stored.extend(entries.clone());
            Ok(entries)
        })
    }

    fn replace_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        entries: Vec<CalendarEntryView>,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                existing.user_id != user_id || existing.date < oldest || existing.date > newest
            });
            stored.extend(entries.clone());
            Ok(entries)
        })
    }
}

#[derive(Clone, Default)]
struct TestManualCalendarRefreshService;

impl ManualCalendarRefreshUseCases for TestManualCalendarRefreshService {
    fn refresh_calendar_view_for_user(
        &self,
        _user_id: &str,
    ) -> CalendarBoxFuture<Result<ManualCalendarRefreshResult, CalendarEntryViewError>> {
        Box::pin(async {
            Ok(ManualCalendarRefreshResult {
                oldest: "2026-01-01".to_string(),
                newest: "2026-04-27".to_string(),
                rebuilt_entry_count: 3,
            })
        })
    }
}

impl InMemoryCalendarEntryViewRepository {
    pub(crate) fn with_entries(entries: Vec<CalendarEntryView>) -> Self {
        Self {
            stored: Arc::new(std::sync::Mutex::new(entries)),
        }
    }
}

pub(crate) fn sample_calendar_entry(
    entry_id: &str,
    entry_kind: CalendarEntryKind,
    date: &str,
) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: entry_id.to_string(),
        user_id: "user-1".to_string(),
        entry_kind,
        date: date.to_string(),
        start_date_local: Some(format!("{date}T00:00:00")),
        title: format!("Entry {entry_id}"),
        subtitle: None,
        description: None,
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: None,
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: None,
        special_day_id: None,
        race: None,
        summary: None,
        sync: None,
    }
}

pub(crate) fn sample_planned_calendar_entry(
    entry_id: &str,
    date: &str,
    title: &str,
    raw_workout_doc: &str,
) -> CalendarEntryView {
    CalendarEntryView {
        title: title.to_string(),
        raw_workout_doc: Some(raw_workout_doc.to_string()),
        planned_workout_id: Some(entry_id.trim_start_matches("planned:").to_string()),
        sync: Some(CalendarEntrySync {
            linked_intervals_event_id: Some(1),
            sync_status: Some("synced".to_string()),
        }),
        ..sample_calendar_entry(entry_id, CalendarEntryKind::PlannedWorkout, date)
    }
}

struct FrontendFixture {
    root: PathBuf,
}

fn shared_frontend_fixture() -> &'static FrontendFixture {
    SHARED_FRONTEND_FIXTURE.get_or_init(frontend_fixture)
}

fn frontend_fixture() -> FrontendFixture {
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-intervals-spa-fixture-{}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&dist_dir).unwrap();
    fs::write(
        dist_dir.join("index.html"),
        "<!doctype html><html><body><div id=\"root\">fixture</div></body></html>",
    )
    .unwrap();

    FrontendFixture { root }
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }
}

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}
