use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    get_json, session_cookie, settings_test_app_with_athlete_summary, MockLlmChatService,
    TestAthleteSummaryService, TestIdentityServiceWithSession, TestLlmConfigProvider,
    TestSettingsService,
};

#[tokio::test]
async fn get_athlete_summary_requires_authentication() {
    let app = settings_test_app_with_athlete_summary(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
        Some(std::sync::Arc::new(TestAthleteSummaryService::empty())),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/athlete-summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_athlete_summary_returns_empty_state_when_missing() {
    let app = settings_test_app_with_athlete_summary(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
        Some(std::sync::Arc::new(TestAthleteSummaryService::empty())),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/athlete-summary")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(body.get("exists").and_then(Value::as_bool), Some(false));
    assert_eq!(body.get("stale").and_then(Value::as_bool), Some(true));
}

#[tokio::test]
async fn generate_athlete_summary_returns_generated_summary() {
    let app = settings_test_app_with_athlete_summary(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
        Some(std::sync::Arc::new(TestAthleteSummaryService::empty())),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/athlete-summary/generate")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(body.get("exists").and_then(Value::as_bool), Some(true));
    assert_eq!(body.get("summaryText").and_then(Value::as_str), Some("OK"));
}
