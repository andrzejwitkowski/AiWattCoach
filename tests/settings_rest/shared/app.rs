use std::{
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        identity::IdentityUseCases, intervals::IntervalsConnectionTester,
        settings::UserSettingsUseCases,
    },
    Settings,
};
use axum::{body::to_bytes, http::header};
use mongodb::Client;

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn session_cookie(value: &str) -> header::HeaderValue {
    header::HeaderValue::from_str(&format!("aiwattcoach_session={value}; Path=/")).unwrap()
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

pub(crate) async fn settings_test_app(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
) -> axum::Router {
    settings_test_app_with_intervals(identity_service, settings_service, None).await
}

pub(crate) async fn settings_test_app_with_intervals(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
    intervals_connection_tester: Option<std::sync::Arc<dyn IntervalsConnectionTester>>,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();

    let mut app_state = AppState::new(
        settings.app_name,
        settings.mongo.database,
        test_mongo_client(&settings.mongo.uri).await,
    )
    .with_identity_service(
        std::sync::Arc::new(identity_service),
        "aiwattcoach_session",
        "lax",
        false,
        24,
    )
    .with_settings_service(std::sync::Arc::new(settings_service));

    if let Some(tester) = intervals_connection_tester {
        app_state = app_state.with_intervals_connection_tester(tester);
    }

    build_app_with_frontend_dist(app_state, fixture.dist_dir())
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
        "aiwattcoach-settings-spa-fixture-{}-{unique}-{counter}",
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

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}
