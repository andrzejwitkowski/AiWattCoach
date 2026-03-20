use aiwattcoach::{build_app, AppState, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_check_returns_service_status() {
    let settings = Settings::test_defaults();
    let app = build_app(AppState::without_mongo(settings));

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
    assert_eq!(payload["mongo_configured"], false);
}

#[tokio::test]
async fn readiness_returns_service_unavailable_without_mongo() {
    let settings = Settings::test_defaults();
    let app = build_app(AppState::without_mongo(settings));

    let response = app
        .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "degraded");
    assert_eq!(payload["reason"], "mongo_unavailable");
}
