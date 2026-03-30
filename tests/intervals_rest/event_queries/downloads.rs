use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::{
    app::{intervals_test_app, RESPONSE_LIMIT_BYTES},
    fixtures::{sample_event, session_cookie},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::{ScopedIntervalsService, TestIntervalsService},
};

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
