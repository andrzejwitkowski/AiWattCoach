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
    domain::identity::{AppUser, IdentityUseCases, Role},
    domain::settings::{
        AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases,
    },
    Settings,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use mongodb::Client;
use serde_json::Value;
use tower::util::ServiceExt;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn session_cookie(value: &str) -> header::HeaderValue {
    header::HeaderValue::from_str(&format!("aiwattcoach_session={value}; Path=/")).unwrap()
}

async fn get_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    let parts = response.into_parts();
    let body = to_bytes(parts.1, RESPONSE_LIMIT_BYTES)
        .await
        .expect("body to be collected");
    serde_json::from_slice(&body).expect("valid JSON")
}

async fn settings_test_app(
    identity_service: impl IdentityUseCases + 'static,
    settings_service: impl UserSettingsUseCases + 'static,
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
            std::sync::Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_settings_service(std::sync::Arc::new(settings_service)),
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

#[derive(Clone)]
struct TestSettingsService {
    settings: Option<UserSettings>,
}

impl TestSettingsService {
    fn new() -> Self {
        Self { settings: None }
    }

    fn with_settings(settings: UserSettings) -> Self {
        Self { settings: Some(settings) }
    }
}

impl Default for TestSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserSettingsUseCases for TestSettingsService {
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(settings.unwrap_or_else(|| {
                UserSettings::new_defaults(user_id, 1000)
            }))
        })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1000);
            settings.ai_agents = ai_agents;
            settings.updated_at_epoch_seconds = 2000;
            Ok(settings)
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1000);
            settings.intervals = intervals;
            settings.updated_at_epoch_seconds = 2000;
            Ok(settings)
        })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1000);
            settings.options = options;
            settings.updated_at_epoch_seconds = 2000;
            Ok(settings)
        })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = UserSettings::new_defaults(user_id, 1000);
            settings.cycling = cycling;
            settings.updated_at_epoch_seconds = 2000;
            Ok(settings)
        })
    }
}

#[tokio::test]
async fn get_settings_requires_authentication() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_settings_returns_default_settings_for_authenticated_user() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(body.get("aiAgents").is_some());
    assert!(body.get("intervals").is_some());
    assert!(body.get("options").is_some());
    assert!(body.get("cycling").is_some());

    let ai_agents = body.get("aiAgents").unwrap();
    assert_eq!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap(), false);
    assert_eq!(ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap(), false);

    let intervals = body.get("intervals").unwrap();
    assert_eq!(intervals.get("connected").unwrap().as_bool().unwrap(), false);

    let options = body.get("options").unwrap();
    assert_eq!(options.get("analyzeWithoutHeartRate").unwrap().as_bool().unwrap(), false);
}

#[tokio::test]
async fn get_settings_masks_api_keys() {
    let settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    let mut settings_with_keys = settings;
    settings_with_keys.ai_agents.openai_api_key = Some("sk-verysecretkey1234".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings_with_keys),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    let ai_agents = body.get("aiAgents").unwrap();

    assert_eq!(ai_agents.get("openaiApiKey").unwrap().as_str().unwrap(), "***...1234");
    assert_eq!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap(), true);
    assert!(ai_agents.get("geminiApiKey").is_none() || ai_agents.get("geminiApiKey").unwrap().is_null());
}

#[tokio::test]
async fn update_ai_agents_saves_and_returns_updated_settings() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-new-openai-key",
        "geminiApiKey": "AIza-new-gemini-key"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    let openai_masked = ai_agents.get("openaiApiKey").unwrap().as_str().unwrap();
    let gemini_masked = ai_agents.get("geminiApiKey").unwrap().as_str().unwrap();

    assert!(openai_masked.starts_with("***..."), "expected openaiApiKey to be masked, got: {}", openai_masked);
    assert!(gemini_masked.starts_with("***..."), "expected geminiApiKey to be masked, got: {}", gemini_masked);
    assert!(!gemini_masked.ends_with("ey-1"), "gemini should not mask to ey-1");
    assert_eq!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap(), true);
    assert_eq!(ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap(), true);
}

#[tokio::test]
async fn update_intervals_saves_athlete_id() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "apiKey": "intervals-api-key-xyz",
        "athleteId": "i12345678"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/intervals")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let intervals = response_body.get("intervals").unwrap();

    assert_eq!(intervals.get("athleteId").unwrap().as_str().unwrap(), "i12345678");
    assert_eq!(intervals.get("apiKeySet").unwrap().as_bool().unwrap(), true);
}

#[tokio::test]
async fn update_options_sets_analyze_without_heart_rate() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "analyzeWithoutHeartRate": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/options")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let options = response_body.get("options").unwrap();

    assert_eq!(options.get("analyzeWithoutHeartRate").unwrap().as_bool().unwrap(), true);
}

#[tokio::test]
async fn update_cycling_saves_biometrics() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "fullName": "Alex Rivier",
        "age": 28,
        "heightCm": 182,
        "weightKg": 74.0,
        "ftpWatts": 280,
        "hrMaxBpm": 192,
        "vo2Max": 58.0
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let cycling = response_body.get("cycling").unwrap();

    assert_eq!(cycling.get("fullName").unwrap().as_str().unwrap(), "Alex Rivier");
    assert_eq!(cycling.get("age").unwrap().as_i64().unwrap(), 28);
    assert_eq!(cycling.get("heightCm").unwrap().as_i64().unwrap(), 182);
    assert_eq!(cycling.get("weightKg").unwrap().as_f64().unwrap(), 74.0);
    assert_eq!(cycling.get("ftpWatts").unwrap().as_i64().unwrap(), 280);
    assert_eq!(cycling.get("hrMaxBpm").unwrap().as_i64().unwrap(), 192);
    assert_eq!(cycling.get("vo2Max").unwrap().as_f64().unwrap(), 58.0);
}

#[derive(Clone, Default)]
struct TestIdentityServiceWithSession {
    session_id: String,
    user_id: String,
    roles: Vec<Role>,
}

impl TestIdentityServiceWithSession {
    fn session(session_id: &str, user_id: &str, roles: Vec<Role>) -> Self {
        Self {
            session_id: session_id.to_string(),
            user_id: user_id.to_string(),
            roles,
        }
    }
}

impl IdentityUseCases for TestIdentityServiceWithSession {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<Result<aiwattcoach::domain::identity::GoogleLoginStart, aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Ok(aiwattcoach::domain::identity::GoogleLoginStart { state: "state-1".to_string(), redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1".to_string() }) })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<aiwattcoach::domain::identity::GoogleLoginSuccess, aiwattcoach::domain::identity::IdentityError>> {
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            Ok(aiwattcoach::domain::identity::GoogleLoginSuccess {
                user: AppUser::new(
                    user_id.clone(),
                    "google-subject-1".to_string(),
                    "athlete@example.com".to_string(),
                    roles,
                    Some("Test User".to_string()),
                    None,
                    true,
                ),
                session: aiwattcoach::domain::identity::AuthSession::new(
                    session_id,
                    user_id,
                    999999,
                    100,
                ),
                redirect_to: "/app".to_string(),
            })
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, aiwattcoach::domain::identity::IdentityError>> {
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != "session-1" {
                return Ok(None);
            }
            Ok(Some(AppUser::new(
                user_id,
                "google-subject-1".to_string(),
                "athlete@example.com".to_string(),
                roles,
                Some("Test User".to_string()),
                None,
                true,
            )))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Ok(AppUser::new("user-1".to_string(), "google-subject-1".to_string(), "admin@example.com".to_string(), vec![Role::User, Role::Admin], Some("Admin".to_string()), None, true)) })
    }
}
