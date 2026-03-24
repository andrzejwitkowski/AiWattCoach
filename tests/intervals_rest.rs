mod support;

use std::{
    collections::HashMap,
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        identity::{AppUser, IdentityUseCases, Role},
        intervals::{
            CreateEvent, DateRange, Event, EventCategory, IntervalsError, IntervalsUseCases,
            UpdateEvent,
        },
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

use crate::support::tracing_capture::capture_tracing_logs;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn list_events_requires_authentication() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_events_returns_events_for_authenticated_user() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(
            11,
            "VO2 Session",
            Some("- 10min 55%\n- 5x3min 120%".to_string()),
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    let event = &body.as_array().unwrap()[0];
    assert_eq!(event.get("id").unwrap().as_i64().unwrap(), 11);
    assert_eq!(
        event
            .get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        event
            .get("eventDefinition")
            .unwrap()
            .get("rawWorkoutDoc")
            .unwrap()
            .as_str()
            .unwrap(),
        "- 10min 55%\n- 5x3min 120%"
    );
    assert!(event.get("actualWorkout").unwrap().is_null());
}

#[tokio::test]
async fn list_events_returns_422_when_credentials_not_configured() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::CredentialsNotConfigured),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn list_events_are_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    101,
                    "User One Workout",
                    Some("- 1x10min 90%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    202,
                    "User Two Workout",
                    Some("- 4x4min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response_user_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let response_user_2 = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body_user_1: Value = get_json(response_user_1).await;
    let body_user_2: Value = get_json(response_user_2).await;

    assert_eq!(body_user_1.as_array().unwrap().len(), 1);
    assert_eq!(body_user_2.as_array().unwrap().len(), 1);
    assert_eq!(
        body_user_1.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(101)
    );
    assert_eq!(
        body_user_2.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(202)
    );
}

#[tokio::test]
async fn get_event_returns_single_event() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(
            21,
            "Threshold",
            Some("- 20min 95%".to_string()),
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str().unwrap(), "Threshold");
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .get("definition")
            .unwrap()
            .as_str()
            .unwrap(),
        "- 20min 95%"
    );
    assert!(body.get("actualWorkout").unwrap().is_null());
}

#[tokio::test]
async fn get_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    501,
                    "User One Workout",
                    Some("- 1x20min 90%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    502,
                    "User Two Workout",
                    Some("- 6x2min 130%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/502")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_event_returns_404_when_not_found() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_event_returns_201() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let request_body = serde_json::json!({
        "category": "WORKOUT",
        "startDateLocal": "2026-03-25",
        "name": "Sweet Spot",
        "description": "mid-week",
        "indoor": true,
        "color": "green",
        "workoutDoc": "- 15min 88%\n- 5min 55%"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str().unwrap(), "Sweet Spot");
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn create_event_is_scoped_to_authenticated_user() {
    let service = ScopedIntervalsService::with_user_events([
        (
            "user-1",
            vec![sample_event(
                301,
                "User One Existing",
                Some("- 5min 55%".to_string()),
            )],
        ),
        (
            "user-2",
            vec![sample_event(
                401,
                "User Two Existing",
                Some("- 3x3min 120%".to_string()),
            )],
        ),
    ]);
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        service,
    )
    .await;

    let request_body = serde_json::json!({
        "category": "WORKOUT",
        "startDateLocal": "2026-03-26",
        "name": "Created For User One",
        "indoor": true,
        "workoutDoc": "- 2x15min 90%"
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_user_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_user_2 = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let user_1_body: Value = get_json(list_user_1).await;
    let user_2_body: Value = get_json(list_user_2).await;

    assert_eq!(user_1_body.as_array().unwrap().len(), 2);
    assert_eq!(user_2_body.as_array().unwrap().len(), 1);
    assert_eq!(
        user_2_body.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(401)
    );
}

#[tokio::test]
async fn update_event_returns_200() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(30, "Old", None)]),
    )
    .await;

    let request_body = serde_json::json!({
        "name": "Updated Workout",
        "indoor": false,
        "workoutDoc": "- 2x20min 90%"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/intervals/events/30")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("name").unwrap().as_str().unwrap(),
        "Updated Workout"
    );
}

#[tokio::test]
async fn update_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    601,
                    "User One Workout",
                    Some("- 5min 55%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    602,
                    "User Two Workout",
                    Some("- 4x4min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let request_body = serde_json::json!({
        "name": "Hijack Attempt",
        "workoutDoc": "- 99min 999w"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/intervals/events/602")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_event_rejects_invalid_category() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let request_body = serde_json::json!({
        "category": "INVALID",
        "startDateLocal": "2026-03-25",
        "name": "Bad",
        "indoor": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_events_rejects_invalid_date_query() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=20260301&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_events_rejects_impossible_calendar_date_query() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-02-31&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_event_returns_204() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(40, "Delete Me", None)]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/intervals/events/40")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    701,
                    "User One Workout",
                    Some("- 5min 55%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    702,
                    "User Two Workout",
                    Some("- 3x3min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/intervals/events/702")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_fit_returns_binary_file() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_fit_bytes(vec![1, 9, 9, 4]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/123/download.fit")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"event-123.fit\""
    );
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), &[1, 9, 9, 4]);
}

#[tokio::test]
async fn download_fit_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    801,
                    "User One Workout",
                    Some("- 5min 55%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    802,
                    "User Two Workout",
                    Some("- 4x4min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/802/download.fit")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "current_thread")]
async fn api_error_returns_502() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::ApiError("upstream failure".to_string())),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/intervals/events/12")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(logs.contains("\"level\":\"ERROR\""), "logs were: {logs}");
    assert!(
        logs.contains("Intervals.icu API error: upstream failure"),
        "logs were: {logs}"
    );
    assert!(
        logs.contains("\"error_chain\":\"Intervals.icu API error: upstream failure\""),
        "logs were: {logs}"
    );
    assert!(logs.contains("\"status\":502"), "logs were: {logs}");
}

#[tokio::test(flavor = "current_thread")]
async fn list_events_returns_422_and_logs_warn_when_credentials_not_configured() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::CredentialsNotConfigured),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(logs.contains("\"level\":\"WARN\""), "logs were: {logs}");
    assert!(
        logs.contains("Intervals.icu credentials are not configured"),
        "logs were: {logs}"
    );
    assert!(logs.contains("\"status\":422"), "logs were: {logs}");
}

async fn intervals_test_app(
    identity_service: impl IdentityUseCases + 'static,
    intervals_service: impl IntervalsUseCases + 'static,
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
        .with_intervals_service(Arc::new(intervals_service)),
        fixture.dist_dir(),
    )
}

fn sample_event(id: i64, name: &str, workout_doc: Option<String>) -> Event {
    Event {
        id,
        start_date_local: "2026-03-22".to_string(),
        name: Some(name.to_string()),
        category: EventCategory::Workout,
        description: Some("structured workout".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc,
    }
}

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

#[derive(Clone, Default)]
struct TestIntervalsService {
    events: Arc<Mutex<Vec<Event>>>,
    fit_bytes: Arc<Vec<u8>>,
    error: Option<IntervalsError>,
}

impl TestIntervalsService {
    fn with_events(events: Vec<Event>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
        }
    }

    fn with_error(error: IntervalsError) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: Some(error),
        }
    }

    fn with_fit_bytes(bytes: Vec<u8>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(bytes),
            error: None,
        }
    }
}

impl IntervalsUseCases for TestIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let error = self.error.clone();
        let events = self.events.lock().unwrap().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(events)
        })
    }

    fn get_event(&self, _user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let events = self.events.lock().unwrap().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            events
                .into_iter()
                .find(|event| event.id == event_id)
                .ok_or(IntervalsError::NotFound)
        })
    }

    fn create_event(
        &self,
        _user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let event = Event {
                id: 1000,
                start_date_local: event.start_date_local,
                name: event.name,
                category: event.category,
                description: event.description,
                indoor: event.indoor,
                color: event.color,
                workout_doc: event.workout_doc,
            };
            store.lock().unwrap().push(event.clone());
            Ok(event)
        })
    }

    fn update_event(
        &self,
        _user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut events = store.lock().unwrap();
            let existing = events
                .iter_mut()
                .find(|existing| existing.id == event_id)
                .ok_or(IntervalsError::NotFound)?;

            if let Some(category) = event.category {
                existing.category = category;
            }
            if let Some(start_date_local) = event.start_date_local {
                existing.start_date_local = start_date_local;
            }
            if let Some(name) = event.name {
                existing.name = Some(name);
            }
            if let Some(description) = event.description {
                existing.description = Some(description);
            }
            if let Some(indoor) = event.indoor {
                existing.indoor = indoor;
            }
            if let Some(color) = event.color {
                existing.color = Some(color);
            }
            if let Some(workout_doc) = event.workout_doc {
                existing.workout_doc = Some(workout_doc);
            }

            Ok(existing.clone())
        })
    }

    fn delete_event(&self, _user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut events = store.lock().unwrap();
            let before = events.len();
            events.retain(|event| event.id != event_id);
            if events.len() == before {
                return Err(IntervalsError::NotFound);
            }
            Ok(())
        })
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let error = self.error.clone();
        let fit_bytes = self.fit_bytes.as_ref().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(fit_bytes)
        })
    }
}

#[derive(Clone)]
struct TestIdentityServiceWithSession {
    session_id: String,
    user_id: String,
    email: String,
    roles: Vec<Role>,
    display_name: String,
}

impl Default for TestIdentityServiceWithSession {
    fn default() -> Self {
        Self {
            session_id: "session-1".to_string(),
            user_id: "user-1".to_string(),
            email: "athlete@example.com".to_string(),
            roles: vec![Role::User],
            display_name: "Test User".to_string(),
        }
    }
}

impl TestIdentityServiceWithSession {
    fn build_user(&self) -> AppUser {
        AppUser::new(
            self.user_id.clone(),
            format!("google-subject-{}", self.user_id),
            self.email.clone(),
            self.roles.clone(),
            Some(self.display_name.clone()),
            None,
            true,
        )
    }
}

#[derive(Clone, Default)]
struct SessionMappedIdentityService {
    users_by_session: HashMap<String, AppUser>,
}

impl SessionMappedIdentityService {
    fn with_users<const N: usize>(entries: [(&str, &str, &str); N]) -> Self {
        let users_by_session = entries
            .into_iter()
            .map(|(session_id, user_id, email)| {
                (
                    session_id.to_string(),
                    AppUser::new(
                        user_id.to_string(),
                        format!("google-subject-{user_id}"),
                        email.to_string(),
                        vec![Role::User],
                        Some(format!("User {user_id}")),
                        None,
                        true,
                    ),
                )
            })
            .collect();

        Self { users_by_session }
    }
}

impl IdentityUseCases for SessionMappedIdentityService {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<
        Result<
            aiwattcoach::domain::identity::GoogleLoginStart,
            aiwattcoach::domain::identity::IdentityError,
        >,
    > {
        Box::pin(async {
            Ok(aiwattcoach::domain::identity::GoogleLoginStart {
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
    ) -> BoxFuture<
        Result<
            aiwattcoach::domain::identity::GoogleLoginSuccess,
            aiwattcoach::domain::identity::IdentityError,
        >,
    > {
        Box::pin(async {
            Err(aiwattcoach::domain::identity::IdentityError::External(
                "not used in test".to_string(),
            ))
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, aiwattcoach::domain::identity::IdentityError>> {
        let user = self.users_by_session.get(session_id).cloned();
        Box::pin(async move { Ok(user) })
    }

    fn logout(
        &self,
        _session_id: &str,
    ) -> BoxFuture<Result<(), aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(
        &self,
        _session_id: &str,
    ) -> BoxFuture<Result<AppUser, aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Err(aiwattcoach::domain::identity::IdentityError::Forbidden) })
    }
}

#[derive(Clone, Default)]
struct ScopedIntervalsService {
    events_by_user: Arc<Mutex<HashMap<String, Vec<Event>>>>,
}

impl ScopedIntervalsService {
    fn with_user_events<const N: usize>(entries: [(&str, Vec<Event>); N]) -> Self {
        let events_by_user = entries
            .into_iter()
            .map(|(user_id, events)| (user_id.to_string(), events))
            .collect();

        Self {
            events_by_user: Arc::new(Mutex::new(events_by_user)),
        }
    }
}

impl IntervalsUseCases for ScopedIntervalsService {
    fn list_events(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            Ok(store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|event| event.id == event_id)
                .ok_or(IntervalsError::NotFound)
        })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let next_id = events.iter().map(|existing| existing.id).max().unwrap_or(0) + 1;
            let event = Event {
                id: next_id,
                start_date_local: event.start_date_local,
                name: event.name,
                category: event.category,
                description: event.description,
                indoor: event.indoor,
                color: event.color,
                workout_doc: event.workout_doc,
            };
            events.push(event.clone());
            Ok(event)
        })
    }

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let existing = events
                .iter_mut()
                .find(|existing| existing.id == event_id)
                .ok_or(IntervalsError::NotFound)?;

            if let Some(category) = event.category {
                existing.category = category;
            }
            if let Some(start_date_local) = event.start_date_local {
                existing.start_date_local = start_date_local;
            }
            if let Some(name) = event.name {
                existing.name = Some(name);
            }
            if let Some(description) = event.description {
                existing.description = Some(description);
            }
            if let Some(indoor) = event.indoor {
                existing.indoor = indoor;
            }
            if let Some(color) = event.color {
                existing.color = Some(color);
            }
            if let Some(workout_doc) = event.workout_doc {
                existing.workout_doc = Some(workout_doc);
            }

            Ok(existing.clone())
        })
    }

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let before = events.len();
            events.retain(|event| event.id != event_id);
            if events.len() == before {
                return Err(IntervalsError::NotFound);
            }
            Ok(())
        })
    }

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let has_event = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .any(|event| event.id == event_id);

            if !has_event {
                return Err(IntervalsError::NotFound);
            }

            Ok(vec![1, 2, 3])
        })
    }
}

impl IdentityUseCases for TestIdentityServiceWithSession {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<
        Result<
            aiwattcoach::domain::identity::GoogleLoginStart,
            aiwattcoach::domain::identity::IdentityError,
        >,
    > {
        Box::pin(async {
            Ok(aiwattcoach::domain::identity::GoogleLoginStart {
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
    ) -> BoxFuture<
        Result<
            aiwattcoach::domain::identity::GoogleLoginSuccess,
            aiwattcoach::domain::identity::IdentityError,
        >,
    > {
        let user_id = self.user_id.clone();
        let session_id = self.session_id.clone();
        let user = self.build_user();
        Box::pin(async move {
            Ok(aiwattcoach::domain::identity::GoogleLoginSuccess {
                user,
                session: aiwattcoach::domain::identity::AuthSession::new(
                    session_id, user_id, 999999, 100,
                ),
                redirect_to: "/app".to_string(),
            })
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, aiwattcoach::domain::identity::IdentityError>> {
        let expected_session_id = self.session_id.clone();
        let user = self.build_user();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != expected_session_id {
                return Ok(None);
            }
            Ok(Some(user))
        })
    }

    fn logout(
        &self,
        _session_id: &str,
    ) -> BoxFuture<Result<(), aiwattcoach::domain::identity::IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<AppUser, aiwattcoach::domain::identity::IdentityError>> {
        let expected_session_id = self.session_id.clone();
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != expected_session_id {
                return Err(aiwattcoach::domain::identity::IdentityError::Unauthenticated);
            }
            if !roles.contains(&Role::Admin) {
                return Err(aiwattcoach::domain::identity::IdentityError::Forbidden);
            }
            Ok(AppUser::new(
                user_id,
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                roles,
                Some("Admin".to_string()),
                None,
                true,
            ))
        })
    }
}
