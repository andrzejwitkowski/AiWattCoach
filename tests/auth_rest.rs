mod support;

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
    domain::identity::{
        AppUser, AuthSession, GoogleLoginStart, GoogleLoginSuccess, IdentityError,
        IdentityUseCases, Role,
    },
    domain::settings::{
        AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError,
        UserSettings, UserSettingsUseCases,
    },
    Settings,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderValue, Request, StatusCode},
};
use mongodb::Client;
use serde_json::Value;
use tower::util::ServiceExt;

use crate::support::tracing_capture::capture_tracing_logs;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
#[tokio::test]
async fn google_start_redirects_to_provider() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/start?returnTo=%2Fsettings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
    );
}

#[tokio::test]
async fn google_start_drops_unsafe_return_to_values() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/start?returnTo=https%3A%2F%2Fevil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
    );
}

#[tokio::test]
async fn google_callback_sets_cookie_and_redirects_into_app() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/app");

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session=session-1"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
}

#[tokio::test]
async fn google_callback_sets_none_same_site_cookie_for_cross_site_mode() {
    let settings = Settings::from_map(&std::collections::BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3002".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
        (
            "GOOGLE_OAUTH_CLIENT_ID".to_string(),
            "client-id.apps.googleusercontent.com".to_string(),
        ),
        (
            "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
            "super-secret".to_string(),
        ),
        (
            "GOOGLE_OAUTH_REDIRECT_URL".to_string(),
            "http://localhost:3002/api/auth/google/callback".to_string(),
        ),
        (
            "SESSION_COOKIE_NAME".to_string(),
            "aiwattcoach_session".to_string(),
        ),
        ("SESSION_COOKIE_SAME_SITE".to_string(), "none".to_string()),
        ("SESSION_TTL_HOURS".to_string(), "24".to_string()),
        ("SESSION_COOKIE_SECURE".to_string(), "true".to_string()),
    ]))
    .unwrap();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            std::sync::Arc::new(TestIdentityService::default()),
            settings.auth.session.cookie_name,
            settings.auth.session.same_site,
            settings.auth.session.secure,
            settings.auth.session.ttl_hours,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("SameSite=None"));
    assert!(cookie.contains("Secure"));
}

#[tokio::test]
async fn me_returns_unauthenticated_without_cookie() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["authenticated"], false);
}

#[tokio::test]
async fn me_returns_current_user_when_cookie_matches_session() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/me")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["authenticated"], true);
    assert_eq!(payload["user"]["email"], "admin@example.com");
    assert_eq!(payload["user"]["roles"][0], "user");
    assert_eq!(payload["user"]["roles"][1], "admin");
}

#[tokio::test]
async fn me_reads_session_cookie_from_later_cookie_header() {
    let app = auth_test_app(TestIdentityService::default()).await;
    let mut request = Request::builder()
        .uri("/api/auth/me")
        .body(Body::empty())
        .unwrap();
    request
        .headers_mut()
        .append(header::COOKIE, HeaderValue::from_static("theme=midnight"));
    request.headers_mut().append(
        header::COOKIE,
        HeaderValue::from_static("aiwattcoach_session=session-1"),
    );

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["authenticated"], true);
    assert_eq!(payload["user"]["email"], "admin@example.com");
}

#[tokio::test]
async fn logout_clears_session_cookie() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn admin_system_info_requires_authentication() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_system_info_rejects_non_admin_user() {
    let app = auth_test_app(TestIdentityService {
        admin_cookie_role: Role::User,
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_system_info_rejects_stale_cookie_as_unauthorized() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=missing-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_system_info_returns_payload_for_admin() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["appName"], "AiWattCoach");
    assert_eq!(payload["mongoDatabase"], "aiwattcoach");
}

#[tokio::test(flavor = "current_thread")]
async fn settings_request_logs_authenticated_user_id_on_request_span() {
    let app =
        auth_test_app_with_settings(TestIdentityService::default(), TestSettingsService).await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    // user_id is pseudonymized via SHA-256 (first 16 hex chars of hash)
    assert!(
        logs.contains("\"user_id\":\"c6c289e49e9c05b2\""),
        "expected request logs to include pseudonymized user_id, got: {logs}"
    );
}

async fn auth_test_app(identity_service: TestIdentityService) -> axum::Router {
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
        ),
        fixture.dist_dir(),
    )
}

async fn auth_test_app_with_settings(
    identity_service: TestIdentityService,
    settings_service: TestSettingsService,
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

#[derive(Default)]
struct TestSettingsService;

impl UserSettingsUseCases for TestSettingsService {
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(UserSettings::new_defaults(user_id, 1000)) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_ai_agents is not used in auth tests") })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_intervals is not used in auth tests") })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_options is not used in auth tests") })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { unreachable!("update_cycling is not used in auth tests") })
    }
}

#[derive(Clone)]
struct TestIdentityService {
    admin_cookie_role: Role,
    callback_error: Option<IdentityError>,
    current_user_error: Option<IdentityError>,
    logout_error: Option<IdentityError>,
    require_admin_error: Option<IdentityError>,
}

impl Default for TestIdentityService {
    fn default() -> Self {
        Self {
            admin_cookie_role: Role::Admin,
            callback_error: None,
            current_user_error: None,
            logout_error: None,
            require_admin_error: None,
        }
    }
}

impl IdentityUseCases for TestIdentityService {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        Box::pin(async {
            Ok(GoogleLoginStart {
                state: "state-1".to_string(),
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                    .to_string(),
            })
        })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>> {
        if let Some(error) = self.callback_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        let role = self.admin_cookie_role.clone();
        Box::pin(async move {
            Ok(GoogleLoginSuccess {
                user: AppUser::new(
                    "user-1".to_string(),
                    "google-subject-1".to_string(),
                    "admin@example.com".to_string(),
                    vec![Role::User, role.clone()],
                    Some("Admin Athlete".to_string()),
                    Some("https://example.com/avatar.png".to_string()),
                    true,
                ),
                session: AuthSession::new(
                    "session-1".to_string(),
                    "user-1".to_string(),
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
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        if let Some(error) = self.current_user_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        let role = self.admin_cookie_role.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != "session-1" {
                return Ok(None);
            }

            let mut roles = vec![Role::User];
            if role == Role::Admin {
                roles.push(Role::Admin);
            }

            Ok(Some(AppUser::new(
                "user-1".to_string(),
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                roles,
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
                true,
            )))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        if let Some(error) = self.logout_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        if let Some(error) = self.require_admin_error.clone() {
            return Box::pin(async move { Err(error) });
        }

        let role = self.admin_cookie_role.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != "session-1" {
                return Err(IdentityError::Unauthenticated);
            }

            if role != Role::Admin {
                return Err(IdentityError::Forbidden);
            }

            Ok(AppUser::new(
                "user-1".to_string(),
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                vec![Role::User, Role::Admin],
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
                true,
            ))
        })
    }
}

#[tokio::test]
async fn google_callback_returns_bad_request_for_invalid_login_state() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::InvalidLoginState),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn google_callback_returns_service_unavailable_for_provider_failures() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::External("google timeout".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn me_returns_service_unavailable_when_identity_backend_errors() {
    let app = auth_test_app(TestIdentityService {
        current_user_error: Some(IdentityError::Repository("mongo down".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/me")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn logout_returns_service_unavailable_when_session_invalidation_fails() {
    let app = auth_test_app(TestIdentityService {
        logout_error: Some(IdentityError::Repository("mongo down".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn logout_returns_service_unavailable_and_clears_cookie_without_identity_service() {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn admin_system_info_returns_service_unavailable_for_backend_errors() {
    let app = auth_test_app(TestIdentityService {
        require_admin_error: Some(IdentityError::Repository("mongo down".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FRONTEND_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-auth-spa-fixture-{}-{unique}-{counter}",
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

struct FrontendFixture {
    root: PathBuf,
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
