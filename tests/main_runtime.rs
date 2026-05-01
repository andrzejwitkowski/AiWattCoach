use std::{
    error::Error,
    io::{Error as IoError, Write},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        provider_poll_states::MongoProviderPollStateRepository,
        settings::MongoUserSettingsRepository,
    },
    domain::{
        external_sync::{
            ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
        },
        identity::Clock,
        settings::UserSettingsRepository,
        wahoo::{
            BoxFuture as WahooBoxFuture, WahooAuthExchange, WahooAuthStart, WahooCreatePlan,
            WahooCreateWorkout, WahooError, WahooPlan, WahooToken, WahooUpdatePlan,
            WahooUpdateWorkout, WahooUseCases, WahooUser, WahooWorkout, WahooWorkoutList,
            WahooWorkoutSummary,
        },
    },
    main_runtime::{
        finish_server_shutdown, park_wahoo_poll_states, reconcile_intervals_poll_states,
        reconcile_wahoo_user_ids, should_reset_poll_state, wait_for_ctrl_c,
    },
};
use mongodb::{bson::doc, options::ClientOptions, Client};
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};

#[cfg(unix)]
use aiwattcoach::main_runtime::wait_for_sigterm;

#[derive(Clone, Default)]
struct SharedLogBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedLogBuffer {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("log buffer mutex poisoned").clone())
            .expect("log buffer contained invalid utf-8")
    }
}

impl Write for SharedLogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("log buffer mutex poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
    type Writer = SharedLogBuffer;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[derive(Clone)]
struct FixedClock(i64);

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.0
    }
}

#[derive(Clone)]
struct RecordingWahooService {
    authenticated_users: Arc<Mutex<Vec<String>>>,
}

impl Default for RecordingWahooService {
    fn default() -> Self {
        Self {
            authenticated_users: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl RecordingWahooService {
    fn authenticated_users(&self) -> Vec<String> {
        self.authenticated_users.lock().unwrap().clone()
    }
}

impl WahooUseCases for RecordingWahooService {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> WahooBoxFuture<Result<WahooAuthStart, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> WahooBoxFuture<Result<WahooAuthExchange, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn ensure_token(&self, _user_id: &str) -> WahooBoxFuture<Result<WahooToken, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn get_authenticated_user(
        &self,
        user_id: &str,
    ) -> WahooBoxFuture<Result<WahooUser, WahooError>> {
        let authenticated_users = self.authenticated_users.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            authenticated_users.lock().unwrap().push(user_id);
            Ok(WahooUser { id: 60_462 })
        })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        _page: usize,
        _per_page: usize,
    ) -> WahooBoxFuture<Result<WahooWorkoutList, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> WahooBoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: WahooCreatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        _plan_id: i64,
        _request: WahooUpdatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: WahooCreateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
        _request: WahooUpdateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> WahooBoxFuture<Result<Vec<u8>, WahooError>> {
        Box::pin(async { unreachable!("not used in main_runtime tests") })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_c_registration_error_logs_and_finishes_shutdown_future() {
    let logs = SharedLogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_writer(logs.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let result = timeout(
        Duration::from_millis(50),
        wait_for_ctrl_c(
            async { Err(IoError::other("boom")) },
            Arc::new(Notify::new()),
        ),
    )
    .await;

    assert!(result.is_ok());
    let output = logs.contents();
    assert!(output
        .lines()
        .any(|line| { line.contains("Failed to listen for Ctrl+C") && line.contains("boom") }));
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn sigterm_registration_error_logs_and_finishes_shutdown_future() {
    let logs = SharedLogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_writer(logs.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);

    let result = timeout(
        Duration::from_millis(50),
        wait_for_sigterm(Err(IoError::other("boom")), Arc::new(Notify::new())),
    )
    .await;

    assert!(result.is_ok());
    let output = logs.contents();
    assert!(output
        .lines()
        .any(|line| { line.contains("Failed to listen for SIGTERM") && line.contains("boom") }));
}

#[test]
fn finish_server_shutdown_returns_ok_when_both_succeed() {
    assert!(finish_server_shutdown(Ok(()), Ok(())).is_ok());
}

#[test]
fn finish_server_shutdown_returns_telemetry_error_when_server_succeeds() {
    let error = finish_server_shutdown(Ok(()), Err(Box::new(IoError::other("telemetry boom"))))
        .expect_err("telemetry error should be returned");

    assert!(error.to_string().contains("telemetry boom"));
}

#[test]
fn finish_server_shutdown_combines_server_and_telemetry_errors() {
    let telemetry_error: Box<dyn Error + Send + Sync> = Box::new(IoError::other("telemetry boom"));
    let error = finish_server_shutdown(Err(IoError::other("server boom")), Err(telemetry_error))
        .expect_err("combined error should be returned");

    assert!(error.to_string().contains("server boom"));
    assert!(error.to_string().contains("telemetry boom"));
}

#[test]
fn should_reset_poll_state_when_existing_state_is_missing() {
    assert!(should_reset_poll_state(None, None));
}

#[test]
fn should_not_reset_poll_state_without_intervals_update_timestamp() {
    let state = ProviderPollState {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Intervals,
        stream: ProviderPollStream::Calendar,
        cursor: Some("2026-05-01".to_string()),
        next_due_at_epoch_seconds: i64::MAX,
        last_attempted_at_epoch_seconds: Some(100),
        last_successful_at_epoch_seconds: Some(100),
        last_error: None,
        backoff_until_epoch_seconds: None,
    };

    assert!(!should_reset_poll_state(Some(&state), None));
}

#[test]
fn should_not_reset_poll_state_when_intervals_timestamp_matches_latest_touch() {
    let state = ProviderPollState {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Intervals,
        stream: ProviderPollStream::Calendar,
        cursor: Some("2026-05-01".to_string()),
        next_due_at_epoch_seconds: i64::MAX,
        last_attempted_at_epoch_seconds: Some(100),
        last_successful_at_epoch_seconds: Some(100),
        last_error: None,
        backoff_until_epoch_seconds: None,
    };

    assert!(!should_reset_poll_state(Some(&state), Some(100)));
}

#[test]
fn should_reset_poll_state_when_intervals_timestamp_is_newer_than_latest_touch() {
    let state = ProviderPollState {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Intervals,
        stream: ProviderPollStream::Calendar,
        cursor: Some("2026-05-01".to_string()),
        next_due_at_epoch_seconds: i64::MAX,
        last_attempted_at_epoch_seconds: Some(100),
        last_successful_at_epoch_seconds: Some(100),
        last_error: None,
        backoff_until_epoch_seconds: None,
    };

    assert!(should_reset_poll_state(Some(&state), Some(101)));
}

#[test]
fn should_reset_poll_state_compares_against_latest_poll_touch_timestamp() {
    let state = ProviderPollState {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Intervals,
        stream: ProviderPollStream::Calendar,
        cursor: Some("2026-05-01".to_string()),
        next_due_at_epoch_seconds: i64::MAX,
        last_attempted_at_epoch_seconds: Some(120),
        last_successful_at_epoch_seconds: Some(100),
        last_error: Some("upstream timeout".to_string()),
        backoff_until_epoch_seconds: Some(180),
    };

    assert!(!should_reset_poll_state(Some(&state), Some(110)));
    assert!(should_reset_poll_state(Some(&state), Some(121)));
}

#[tokio::test]
async fn reconcile_intervals_poll_states_seeds_missing_states_for_existing_connected_users() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("main-reconcile-poll-states");
    let settings_repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let poll_states = MongoProviderPollStateRepository::new(client.clone(), &database_name);
    settings_repository.ensure_indexes().await.unwrap();
    poll_states.ensure_indexes().await.unwrap();

    let settings_collection = client
        .database(&database_name)
        .collection::<mongodb::bson::Document>("user_settings");
    settings_collection
        .insert_many([
            doc! {
                "user_id": "connected-user",
                "ai_agents": {},
                "intervals": {
                    "api_key": "api-key",
                    "athlete_id": "athlete-1",
                    "connected": true,
                },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 1,
            },
            doc! {
                "user_id": "legacy-user",
                "ai_agents": {},
                "intervals": {
                    "api_key": "legacy-key",
                    "athlete_id": "legacy-athlete",
                },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 1,
            },
            doc! {
                "user_id": "disconnected-user",
                "ai_agents": {},
                "intervals": {},
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 1,
            },
        ])
        .await
        .unwrap();

    poll_states
        .upsert(ProviderPollState::new(
            "connected-user".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_700_000_000,
        ))
        .await
        .unwrap();
    poll_states
        .upsert(ProviderPollState {
            user_id: "disconnected-user".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::CompletedWorkouts,
            cursor: Some("2026-04-01".to_string()),
            next_due_at_epoch_seconds: 1,
            last_attempted_at_epoch_seconds: Some(1),
            last_successful_at_epoch_seconds: Some(1),
            last_error: Some("should be cleared".to_string()),
            backoff_until_epoch_seconds: Some(2),
        })
        .await
        .unwrap();
    poll_states
        .upsert(ProviderPollState {
            user_id: "connected-user".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::Calendar,
            cursor: Some("2026-04-03".to_string()),
            next_due_at_epoch_seconds: 42,
            last_attempted_at_epoch_seconds: Some(40),
            last_successful_at_epoch_seconds: Some(41),
            last_error: Some("legacy calendar state".to_string()),
            backoff_until_epoch_seconds: Some(99),
        })
        .await
        .unwrap();

    reconcile_intervals_poll_states(
        &settings_repository,
        &poll_states,
        &FixedClock(1_700_000_000),
    )
    .await
    .unwrap();

    let connected_completed = poll_states
        .find_by_provider_and_stream(
            "connected-user",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .expect("connected user completed state should exist");
    assert_eq!(connected_completed.next_due_at_epoch_seconds, 1_700_000_000);
    assert_eq!(connected_completed.cursor, None);
    assert_eq!(connected_completed.last_error, None);
    assert_eq!(connected_completed.backoff_until_epoch_seconds, None);

    let legacy_completed = poll_states
        .find_by_provider_and_stream(
            "legacy-user",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .expect("legacy user completed state should be seeded");
    assert_eq!(legacy_completed.next_due_at_epoch_seconds, 1_700_000_000);

    assert!(poll_states
        .find_by_provider_and_stream(
            "legacy-user",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .is_none());

    let connected_calendar = poll_states
        .find_by_provider_and_stream(
            "connected-user",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .expect("connected user legacy calendar state should be parked");
    assert_eq!(connected_calendar.next_due_at_epoch_seconds, i64::MAX);
    assert_eq!(connected_calendar.cursor, None);
    assert_eq!(connected_calendar.last_error, None);
    assert_eq!(connected_calendar.backoff_until_epoch_seconds, None);

    let disconnected_completed = poll_states
        .find_by_provider_and_stream(
            "disconnected-user",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .expect("disconnected user completed state should still exist");
    assert_eq!(disconnected_completed.next_due_at_epoch_seconds, i64::MAX);
    assert_eq!(disconnected_completed.cursor, None);
    assert_eq!(disconnected_completed.last_error, None);
    assert_eq!(disconnected_completed.backoff_until_epoch_seconds, None);

    let _ = client.database(&database_name).drop().await;
}

#[tokio::test]
async fn park_wahoo_poll_states_disables_legacy_wahoo_completed_polling() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("main-park-wahoo-poll-states");
    let poll_states = MongoProviderPollStateRepository::new(client.clone(), &database_name);
    poll_states.ensure_indexes().await.unwrap();

    poll_states
        .upsert(ProviderPollState {
            user_id: "connected-user".to_string(),
            provider: ExternalProvider::Wahoo,
            stream: ProviderPollStream::CompletedWorkouts,
            cursor: Some("2026-04-03T10:00:00+00:00".to_string()),
            next_due_at_epoch_seconds: 42,
            last_attempted_at_epoch_seconds: Some(40),
            last_successful_at_epoch_seconds: Some(41),
            last_error: Some("legacy wahoo state".to_string()),
            backoff_until_epoch_seconds: Some(99),
        })
        .await
        .unwrap();

    park_wahoo_poll_states(&poll_states).await.unwrap();

    let parked = poll_states
        .find_by_provider_and_stream(
            "connected-user",
            ExternalProvider::Wahoo,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .expect("legacy wahoo completed state should be parked");
    assert_eq!(parked.next_due_at_epoch_seconds, i64::MAX);
    assert_eq!(parked.cursor, None);
    assert_eq!(parked.last_error, None);
    assert_eq!(parked.backoff_until_epoch_seconds, None);

    let _ = client.database(&database_name).drop().await;
}

#[tokio::test]
async fn reconcile_wahoo_user_ids_backfills_missing_ids_for_connected_users() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("main-reconcile-wahoo-user-ids");
    let settings_repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    settings_repository.ensure_indexes().await.unwrap();

    let settings_collection = client
        .database(&database_name)
        .collection::<mongodb::bson::Document>("user_settings");
    settings_collection
        .insert_many([
            doc! {
                "user_id": "legacy-user",
                "ai_agents": {},
                "wahoo": {
                    "access_token": "access-token",
                    "refresh_token": "refresh-token",
                    "connected": true,
                },
                "intervals": {
                    "updated_at_epoch_seconds": 123,
                },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 1,
            },
            doc! {
                "user_id": "already-mapped-user",
                "ai_agents": {},
                "wahoo": {
                    "access_token": "access-token",
                    "refresh_token": "refresh-token",
                    "user_id": 777,
                    "connected": true,
                },
                "intervals": {
                    "updated_at_epoch_seconds": 456,
                },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 2,
            },
        ])
        .await
        .unwrap();

    let wahoo_service = RecordingWahooService::default();
    reconcile_wahoo_user_ids(
        &settings_repository,
        &wahoo_service,
        &FixedClock(1_700_000_000),
    )
    .await
    .unwrap();

    let legacy_user = settings_repository
        .find_by_user_id("legacy-user")
        .await
        .unwrap()
        .expect("legacy user should exist");
    assert_eq!(legacy_user.wahoo.user_id, Some(60_462));
    assert_eq!(legacy_user.updated_at_epoch_seconds, 1);
    assert_eq!(legacy_user.intervals.api_key, None);
    assert_eq!(legacy_user.intervals.athlete_id, None);
    assert_eq!(
        legacy_user.wahoo.updated_at_epoch_seconds,
        Some(1_700_000_000)
    );

    let already_mapped_user = settings_repository
        .find_by_user_id("already-mapped-user")
        .await
        .unwrap()
        .expect("already mapped user should exist");
    assert_eq!(already_mapped_user.wahoo.user_id, Some(777));
    assert_eq!(already_mapped_user.updated_at_epoch_seconds, 2);
    assert_eq!(already_mapped_user.wahoo.updated_at_epoch_seconds, None);
    assert_eq!(
        wahoo_service.authenticated_users(),
        vec!["legacy-user".to_string()]
    );

    let _ = client.database(&database_name).drop().await;
}

async fn test_mongo_client_or_skip() -> Option<Client> {
    let uri =
        std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let mut options = ClientOptions::parse(&uri)
        .await
        .expect("test mongo client should parse uri");
    options.server_selection_timeout = Some(Duration::from_secs(2));
    let client = Client::with_options(options).expect("test mongo client should be created");
    match client
        .database("admin")
        .run_command(doc! { "ping": 1 })
        .await
    {
        Ok(_) => Some(client),
        Err(error) => {
            let message = format!("failed to connect to Mongo: {error}");
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("main_runtime test requires Mongo in CI: {message}");
            }
            eprintln!("skipping main_runtime mongo test: {message}");
            None
        }
    }
}

fn unique_test_database_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}
