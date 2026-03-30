use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{assert_html_response, health_test_app, RESPONSE_LIMIT_BYTES};

#[tokio::test]
async fn health_check_returns_service_status() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/health")
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

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
}

#[tokio::test]
async fn readiness_returns_service_unavailable_without_mongo() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "degraded");
    assert_eq!(payload["reason"], "mongo_unreachable");
}

#[tokio::test]
async fn root_serves_spa_html() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_html_response(response, &expected_html).await;
}

#[tokio::test]
async fn built_frontend_fixture_serves_spa_at_root_while_health_stays_json() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let root_response = test_app
        .app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_html_response(root_response, &expected_html).await;

    let health_response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(health_response.status(), StatusCode::OK);

    let content_type = health_response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("health response should include a content type");
    assert!(
        content_type.starts_with("application/json"),
        "expected JSON content type, got {content_type}"
    );

    let body = to_bytes(health_response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
}
