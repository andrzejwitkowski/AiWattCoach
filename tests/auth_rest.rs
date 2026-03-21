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

async fn auth_test_app(identity_service: TestIdentityService) -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(std::sync::Arc::new(identity_service), "aiwattcoach_session", false),
        fixture.dist_dir(),
    )
}

#[derive(Clone)]
struct TestIdentityService {
    admin_cookie_role: Role,
}

impl Default for TestIdentityService {
    fn default() -> Self {
        Self {
            admin_cookie_role: Role::Admin,
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
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1".to_string(),
            })
        })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>> {
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
                session: AuthSession::new("session-1".to_string(), "user-1".to_string(), 999999, 100),
                redirect_to: "/app".to_string(),
            })
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
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
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        let role = self.admin_cookie_role.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != "session-1" {
                return Err(IdentityError::Forbidden);
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
