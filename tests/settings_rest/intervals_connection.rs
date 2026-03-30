use aiwattcoach::domain::{
    identity::Role, intervals::IntervalsConnectionError, settings::UserSettings,
};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    get_json, session_cookie, settings_test_app_with_intervals, MockIntervalsConnectionTester,
    RepositoryErrorSettingsService, TestIdentityServiceWithSession, TestSettingsService,
};

#[tokio::test]
async fn test_intervals_connection_requires_authentication() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_ok(),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"apiKey":"key","athleteId":"id"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_intervals_connection_returns_200_on_success() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_ok(),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"apiKey":"valid-key","athleteId":"athlete-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(body.get("connected").unwrap().as_bool().unwrap());
    assert_eq!(
        body.get("message").unwrap().as_str().unwrap(),
        "Connection successful."
    );
    assert!(!body.get("usedSavedApiKey").unwrap().as_bool().unwrap());
    assert!(!body.get("usedSavedAthleteId").unwrap().as_bool().unwrap());
    assert!(!body
        .get("persistedStatusUpdated")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn test_intervals_connection_returns_400_on_unauthenticated() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_err(IntervalsConnectionError::Unauthenticated),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"apiKey":"bad-key","athleteId":"athlete-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = get_json(response).await;
    assert!(!body.get("connected").unwrap().as_bool().unwrap());
    assert!(body
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("Invalid API key"));
}

#[tokio::test]
async fn test_intervals_connection_returns_400_on_invalid_configuration() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_err(
                IntervalsConnectionError::InvalidConfiguration,
            ),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"apiKey":"valid-key","athleteId":"bad-athlete"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = get_json(response).await;
    assert!(!body.get("connected").unwrap().as_bool().unwrap());
    assert!(body
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("Invalid configuration"));
}

#[tokio::test]
async fn test_intervals_connection_uses_saved_credentials_when_transient_missing() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = Some("saved-athlete-id".to_string());

    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_ok(),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"apiKey":"","athleteId":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(body.get("connected").unwrap().as_bool().unwrap());
    assert!(body.get("usedSavedApiKey").unwrap().as_bool().unwrap());
    assert!(body.get("usedSavedAthleteId").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn test_intervals_connection_returns_200_when_credentials_incomplete() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_ok(),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"apiKey":"only-api-key"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(!body.get("connected").unwrap().as_bool().unwrap());
    assert!(body
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("Both API key and athlete ID are required"));
}

#[tokio::test]
async fn test_intervals_connection_incomplete_uses_saved_flags_when_available() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = None;

    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_ok(),
        )),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"apiKey":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(!body.get("connected").unwrap().as_bool().unwrap());
    assert!(body.get("usedSavedApiKey").unwrap().as_bool().unwrap());
    assert!(!body.get("usedSavedAthleteId").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn admin_settings_repository_error_still_returns_503() {
    let app = crate::shared::settings_test_app(
        TestIdentityServiceWithSession {
            roles: vec![Role::User, Role::Admin],
            ..Default::default()
        },
        RepositoryErrorSettingsService::new("admin settings repository unavailable"),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/settings/user-999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
