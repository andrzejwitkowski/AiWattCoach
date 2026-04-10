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
    intervals_test_app_with_projections(
        identity_service,
        intervals_service,
        EmptyTrainingPlanProjectionRepository,
    )
    .await
}

pub(crate) async fn intervals_test_app_with_projections(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let calendar_service = Arc::new(CalendarService::new(
        intervals_service.clone(),
        projections,
        InMemoryPlannedWorkoutSyncRepository,
        EmptyHiddenCalendarEventSource,
        TestClock,
    ));
    let calendar_labels_service = Arc::new(CalendarLabelsService::new(EmptyCalendarLabelSource));

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
        .with_intervals_service(Arc::new(intervals_service)),
        fixture.dist_dir(),
    )
}

pub(crate) async fn intervals_test_app_with_all_services(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + Clone + 'static,
    projections: impl TrainingPlanProjectionRepository + Clone + 'static,
    calendar_label_source: impl CalendarLabelSource + Clone + 'static,
    hidden_calendar_event_source: impl HiddenCalendarEventSource + Clone + 'static,
    race_service: impl RaceUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let calendar_service = Arc::new(CalendarService::new(
        intervals_service.clone(),
        projections,
        InMemoryPlannedWorkoutSyncRepository,
        hidden_calendar_event_source,
        TestClock,
    ));
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
struct EmptyHiddenCalendarEventSource;

impl HiddenCalendarEventSource for EmptyHiddenCalendarEventSource {
    fn list_hidden_intervals_event_ids(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> CalendarBoxFuture<Result<Vec<i64>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
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
