use std::{
    fs,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        identity::IdentityUseCases, settings::UserSettingsUseCases,
        workout_summary::WorkoutSummaryUseCases,
    },
    Settings,
};
use axum::body::to_bytes;
use mongodb::Client;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static SHARED_FRONTEND_FIXTURE: OnceLock<FrontendFixture> = OnceLock::new();
static TEST_MONGO_CLIENT: OnceLock<Client> = OnceLock::new();

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
    workout_summary_test_app_with_settings(identity_service, workout_summary_service, None).await
}

pub(crate) async fn workout_summary_test_app_with_settings(
    identity_service: impl IdentityUseCases + 'static,
    workout_summary_service: impl WorkoutSummaryUseCases + 'static,
    settings_service: Option<Arc<dyn UserSettingsUseCases>>,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();

    let mut app_state = AppState::new(
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
    .with_workout_summary_service(Arc::new(workout_summary_service));

    if let Some(settings_service) = settings_service {
        app_state = app_state.with_settings_service(settings_service);
    }

    build_app_with_frontend_dist(app_state, fixture.dist_dir())
}

struct FrontendFixture {
    root: PathBuf,
}

fn shared_frontend_fixture() -> &'static FrontendFixture {
    SHARED_FRONTEND_FIXTURE.get_or_init(frontend_fixture)
}

fn frontend_fixture() -> FrontendFixture {
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-workout-summary-spa-fixture-{}",
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
    if let Some(client) = TEST_MONGO_CLIENT.get() {
        return client.clone();
    }

    let client = Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created");
    let _ = TEST_MONGO_CLIENT.set(client.clone());
    client
}
