use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        calendar::{
            BoxFuture as CalendarBoxFuture, CalendarError, CalendarService,
            HiddenCalendarEventSource, PlannedWorkoutSyncRecord, PlannedWorkoutSyncRepository,
        },
        calendar_labels::{CalendarLabelSource, CalendarLabelsService},
        calendar_view::{
            CalendarEntryKind, CalendarEntrySync, CalendarEntryView, CalendarEntryViewError,
            CalendarEntryViewRepository,
        },
        completed_workouts::{
            CompletedWorkout, CompletedWorkoutError, CompletedWorkoutReadService,
            CompletedWorkoutRepository,
        },
        identity::{Clock, IdentityUseCases},
        intervals::{DateRange, IntervalsUseCases},
        races::RaceUseCases,
        training_plan::{
            BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
            TrainingPlanProjectionRepository, TrainingPlanSnapshot,
        },
    },
    Settings,
};
use mongodb::Client;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    )
    .await
}

pub(crate) async fn intervals_test_app_with_calendar_entries_and_completed_workouts(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_calendar_entries_and_completed_workouts(
        identity_service,
        intervals_service,
        EmptyTrainingPlanProjectionRepository,
        calendar_entry_views,
        completed_workouts,
    )
    .await
}

async fn intervals_test_app_with_projections_calendar_entries_and_completed_workouts(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
) -> axum::Router {
    intervals_test_app_with_projections_and_calendar_entries(
        identity_service,
        intervals_service,
        projections,
        calendar_entry_views,
        completed_workouts,
    )
    .await
}

pub(crate) async fn intervals_test_app_with_projections_and_calendar_entries(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_entry_views: impl CalendarEntryViewRepository + 'static,
    completed_workouts: impl CompletedWorkoutRepository + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let completed_workout_repository = completed_workouts;
    let calendar_service = Arc::new(
        CalendarService::new(
            intervals_service.clone(),
            calendar_entry_views,
            projections,
            InMemoryPlannedWorkoutSyncRepository,
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
        .with_completed_workout_service(completed_workout_service)
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
    let fixture = frontend_fixture();
    let calendar_service = Arc::new(
        CalendarService::new(
            intervals_service.clone(),
            InMemoryCalendarEntryViewRepository::default(),
            projections,
            InMemoryPlannedWorkoutSyncRepository,
            TestClock,
        )
        .with_completed_workouts(InMemoryCompletedWorkoutRepository::default()),
    );
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(calendar_label_source));

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
    ) -> TrainingPlanBoxFuture<
        Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
    > {
        Box::pin(async move { Ok((snapshot, projected_days)) })
    }
}

#[derive(Clone, Default)]
struct InMemoryPlannedWorkoutSyncRepository;

impl PlannedWorkoutSyncRepository for InMemoryPlannedWorkoutSyncRepository {
    fn find_by_user_id_and_projection(
        &self,
        _user_id: &str,
        _operation_key: &str,
        _date: &str,
    ) -> CalendarBoxFuture<Result<Option<PlannedWorkoutSyncRecord>, CalendarError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id_and_range(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> CalendarBoxFuture<Result<Vec<PlannedWorkoutSyncRecord>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> CalendarBoxFuture<Result<PlannedWorkoutSyncRecord, CalendarError>> {
        Box::pin(async move { Ok(record) })
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
            stored.retain(|existing| existing.completed_workout_id != workout.completed_workout_id);
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

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-intervals-spa-fixture-{}-{unique}-{counter}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
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
