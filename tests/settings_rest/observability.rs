use aiwattcoach::domain::{identity::IdentityError, intervals::IntervalsConnectionError};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::{
    shared::{
        assert_log_entry_contains, session_cookie, settings_test_app,
        settings_test_app_with_intervals, AdminIdentityErrorService, MockIntervalsConnectionTester,
        RepositoryErrorSettingsService, TestIdentityServiceWithSession, TestSettingsService,
    },
    tracing_capture::capture_tracing_logs,
};

#[tokio::test(flavor = "current_thread")]
async fn admin_forbidden_logs_warn_before_returning_403() {
    let app = settings_test_app(
        AdminIdentityErrorService::new(IdentityError::Forbidden),
        TestSettingsService::default(),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/admin/settings/user-999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"WARN\"",
            "\"error_kind\":\"forbidden\"",
            "\"status\":403",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn admin_identity_backend_error_logs_error_before_returning_503() {
    let app = settings_test_app(
        AdminIdentityErrorService::new(IdentityError::Repository(
            "identity backend unavailable".to_string(),
        )),
        TestSettingsService::default(),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/admin/settings/user-999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"ERROR\"",
            "\"error_kind\":\"repository_error\"",
            "\"status\":503",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn admin_settings_repository_error_logs_error_kind_before_returning_503() {
    let app = settings_test_app(
        TestIdentityServiceWithSession {
            roles: vec![
                aiwattcoach::domain::identity::Role::User,
                aiwattcoach::domain::identity::Role::Admin,
            ],
            ..Default::default()
        },
        RepositoryErrorSettingsService::new("admin settings repository unavailable"),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/admin/settings/user-999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"ERROR\"",
            "\"error_kind\":\"repository_error\"",
            "\"status\":503",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_intervals_connection_returns_503_on_unavailable() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_err(IntervalsConnectionError::Unavailable),
        )),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
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
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = crate::shared::get_json(response).await;
    assert!(!body.get("connected").unwrap().as_bool().unwrap());
    assert!(body
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("unavailable"));
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"ERROR\"",
            "\"error_kind\":\"unavailable\"",
            "\"status\":503",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn get_settings_returns_503_and_logs_error_kind_on_repository_error() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        RepositoryErrorSettingsService::new("settings repository unavailable"),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"ERROR\"",
            "\"error_kind\":\"repository_error\"",
            "\"status\":503",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn get_settings_logs_redacted_response_body_for_route_with_response_logging() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let (_response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert!(
        logs.contains("\"message\":\"outgoing response\""),
        "expected route-level response body log, got: {logs}"
    );
    assert!(
        logs.contains("\"response_body\":"),
        "expected response body field in logs, got: {logs}"
    );
    assert_log_entry_contains(
        &logs,
        &[
            "\"message\":\"outgoing response\"",
            "\"http.route\":\"/api/settings\"",
            "\"trace_id\":\"",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_intervals_connection_logs_request_body_without_exposing_api_key() {
    let app = settings_test_app_with_intervals(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        Some(std::sync::Arc::new(
            MockIntervalsConnectionTester::returning_err(IntervalsConnectionError::Unavailable),
        )),
    )
    .await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/intervals/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"apiKey":"super-secret-key","athleteId":"athlete-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        logs.contains("\"message\":\"incoming request\""),
        "expected route-level request body log, got: {logs}"
    );
    assert!(
        logs.contains("[REDACTED]"),
        "expected request body redaction, got: {logs}"
    );
    assert!(
        !logs.contains("super-secret-key"),
        "request body log leaked secret, got: {logs}"
    );
    assert_log_entry_contains(
        &logs,
        &[
            "\"message\":\"incoming request\"",
            "\"http.route\":\"/api/settings/intervals/test\"",
            "\"trace_id\":\"",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn update_cycling_returns_400_and_logs_warn_on_validation_error() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "age": 0
    });

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"WARN\"",
            "\"error_kind\":\"validation_error\"",
            "\"status\":400",
        ],
    );
}
