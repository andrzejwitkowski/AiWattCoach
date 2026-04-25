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
    },
    main_runtime::{
        finish_server_shutdown, reconcile_intervals_poll_states, should_reset_poll_state,
        wait_for_ctrl_c,
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
