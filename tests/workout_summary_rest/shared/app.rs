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
    domain::{identity::IdentityUseCases, workout_summary::WorkoutSummaryUseCases},
    Settings,
};
use axum::body::to_bytes;
use mongodb::Client;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn session_cookie(value: &str) -> axum::http::HeaderValue {
    axum::http::HeaderValue::from_str(&format!("aiwattcoach_session={value}; Path=/")).unwrap()
}

pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
    response: axum::response::Response,
) -> T {
    let parts = response.into_parts();
    let body = to_bytes(parts.1, RESPONSE_LIMIT_BYTES)
        .await
        .expect("body to be collected");
    serde_json::from_slice(&body).expect("valid JSON")
}

pub(crate) async fn workout_summary_test_app(
    identity_service: impl IdentityUseCases + 'static,
    workout_summary_service: impl WorkoutSummaryUseCases + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();

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
        .with_workout_summary_service(Arc::new(workout_summary_service)),
        fixture.dist_dir(),
    )
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
        "aiwattcoach-workout-summary-spa-fixture-{}-{unique}-{counter}",
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
