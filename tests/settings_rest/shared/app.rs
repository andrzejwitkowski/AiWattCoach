use std::{fs, future::Future, path::PathBuf, pin::Pin, sync::OnceLock};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        athlete_summary::AthleteSummaryUseCases,
        completed_workouts::CompletedWorkoutAdminUseCases,
        identity::IdentityUseCases,
        intervals::IntervalsConnectionTester,
        llm::{LlmChatPort, UserLlmConfigProvider},
        settings::UserSettingsUseCases,
        wahoo::WahooWebhookUseCases,
    },
    Settings,
};
use axum::{body::to_bytes, http::header};
use mongodb::Client;

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;

static SHARED_FRONTEND_FIXTURE: OnceLock<FrontendFixture> = OnceLock::new();

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
    settings_test_app_with_services(
        identity_service,
        settings_service,
        intervals_connection_tester,
        None,
        None,
    )
    .await
}

pub(crate) async fn settings_test_app_with_services(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
    intervals_connection_tester: Option<std::sync::Arc<dyn IntervalsConnectionTester>>,
    llm_chat_service: Option<std::sync::Arc<dyn LlmChatPort>>,
    llm_config_provider: Option<std::sync::Arc<dyn UserLlmConfigProvider>>,
) -> axum::Router {
    settings_test_app_with_athlete_summary(
        identity_service,
        settings_service,
        intervals_connection_tester,
        llm_chat_service,
        llm_config_provider,
        None,
    )
    .await
}

pub(crate) async fn settings_test_app_with_completed_workout_service(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
    completed_workout_service: impl CompletedWorkoutAdminUseCases + 'static,
) -> axum::Router {
    settings_test_app_with_admin_services(
        identity_service,
        settings_service,
        Some(std::sync::Arc::new(completed_workout_service)),
        None,
    )
    .await
}

pub(crate) async fn settings_test_app_with_admin_services(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
    completed_workout_service: Option<std::sync::Arc<dyn CompletedWorkoutAdminUseCases>>,
    wahoo_webhook_service: Option<std::sync::Arc<dyn WahooWebhookUseCases>>,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();

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

    if let Some(service) = completed_workout_service {
        app_state = app_state.with_completed_workout_admin_service(service);
    }

    if let Some(service) = wahoo_webhook_service {
        app_state = app_state.with_wahoo_webhook_service(service);
    }

    build_app_with_frontend_dist(app_state, fixture.dist_dir())
}

pub(crate) async fn settings_test_app_with_athlete_summary(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
    intervals_connection_tester: Option<std::sync::Arc<dyn IntervalsConnectionTester>>,
    llm_chat_service: Option<std::sync::Arc<dyn LlmChatPort>>,
    llm_config_provider: Option<std::sync::Arc<dyn UserLlmConfigProvider>>,
    athlete_summary_service: Option<std::sync::Arc<dyn AthleteSummaryUseCases>>,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = shared_frontend_fixture();

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

    if let (Some(chat_service), Some(config_provider)) = (llm_chat_service, llm_config_provider) {
        app_state = app_state.with_llm_services(chat_service, config_provider);
    }

    if let Some(service) = athlete_summary_service {
        app_state = app_state.with_athlete_summary_service(service);
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
        "aiwattcoach-settings-spa-fixture-{}",
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
