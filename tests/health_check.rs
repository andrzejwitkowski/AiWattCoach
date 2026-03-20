use aiwattcoach::{build_app, AppState, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_check_returns_service_status() {
    let settings = unreachable_mongo_settings();
    let app = build_app(AppState::new(
        settings.app_name,
        settings.mongo.database,
        test_mongo_client(&settings.mongo.uri).await,
    ));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
}

#[tokio::test]
async fn readiness_returns_service_unavailable_without_mongo() {
    let settings = unreachable_mongo_settings();
    let app = build_app(AppState::new(
        settings.app_name,
        settings.mongo.database,
        test_mongo_client(&settings.mongo.uri).await,
    ));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "degraded");
    assert_eq!(payload["reason"], "mongo_unreachable");
}

async fn test_mongo_client(uri: &str) -> mongodb::Client {
    mongodb::Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}

fn unreachable_mongo_settings() -> Settings {
    let mut settings = Settings::test_defaults();
    settings.mongo.uri = "mongodb://127.0.0.1:27099".to_string();
    settings
}
