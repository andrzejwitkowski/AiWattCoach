mod support;

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{build_app_with_frontend_dist, AppState, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use mongodb::Client;
use serde_json::{json, Value};
use tower::util::ServiceExt;

use crate::support::tracing_capture::capture_tracing_logs;

const RESPONSE_LIMIT_BYTES: usize = 16 * 1024;
static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
#[tokio::test(flavor = "current_thread")]
async fn valid_info_warn_and_error_payloads_are_accepted() {
    let app = logs_test_app().await;

    for (level, expected_level) in [("info", "INFO"), ("warn", "WARN"), ("error", "ERROR")] {
        let message = format!("{level} message from client");
        let (response, logs) = capture_tracing_logs(|| async {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/logs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "level": level,
                                "message": message,
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap()
        })
        .await;

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(
            response_json(response).await,
            json!({ "status": "accepted" })
        );
        assert!(
            logs.contains(&format!("\"client_log_level\":\"{level}\"")),
            "logs were: {logs}"
        );
        assert!(
            logs.contains(&format!("\"client_message\":\"{message}\"")),
            "logs were: {logs}"
        );
        assert!(
            logs.contains(&format!("\"level\":\"{expected_level}\"")),
            "logs were: {logs}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn log_ingestion_returns_not_found_when_disabled() {
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
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "disabled",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(response).await,
        json!({ "error": "disabled" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_level_is_rejected() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "debug",
                        "message": "unsupported",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({ "error": "unsupported_level" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_message_is_rejected() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "x".repeat(10_001),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({ "error": "message_too_long" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn near_limit_valid_payload_is_still_accepted() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "x".repeat(9_900),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response_json(response).await,
        json!({ "status": "accepted" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn escaped_near_limit_valid_payload_is_still_accepted() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "\"".repeat(10_000),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response_json(response).await,
        json!({ "status": "accepted" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_request_body_is_rejected_before_json_parsing() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "\"".repeat(40_000),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

async fn logs_test_app() -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_client_log_ingestion(true),
        fixture.dist_dir(),
    )
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .expect("response body to be collected");
    serde_json::from_slice(&body).expect("response to contain valid JSON")
}

struct FrontendFixture {
    root: PathBuf,
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FRONTEND_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-logs-spa-fixture-{}-{unique}-{counter}",
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
