use aiwattcoach::domain::intervals::IntervalsError;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::{
    app::intervals_test_app, fixtures::session_cookie,
    identity_fakes::TestIdentityServiceWithSession, intervals_fakes::TestIntervalsService,
    support::tracing_capture::capture_tracing_logs, test_support::assert_log_entry_contains,
};

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
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"ERROR\"",
            "\"error_kind\":\"api_error\"",
            "\"status\":502",
        ],
    );
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
    assert_log_entry_contains(
        &logs,
        &[
            "\"level\":\"WARN\"",
            "\"error_kind\":\"credentials_not_configured\"",
            "\"status\":422",
        ],
    );
}
