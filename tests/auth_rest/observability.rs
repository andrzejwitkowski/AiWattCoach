use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::{
    shared::{auth_test_app_with_settings, TestIdentityService, TestSettingsService},
    tracing_capture::capture_tracing_logs,
};

#[tokio::test(flavor = "current_thread")]
async fn settings_request_logs_authenticated_user_id_on_request_span() {
    let app =
        auth_test_app_with_settings(TestIdentityService::default(), TestSettingsService).await;

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        logs.contains("\"user_id\":\"c6c289e49e9c05b2\""),
        "expected request logs to include pseudonymized user_id, got: {logs}"
    );
}
