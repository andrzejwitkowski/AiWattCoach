use aiwattcoach::domain::intervals::IntervalsError;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::intervals_test_app,
    fixtures::{get_json, sample_event, session_cookie},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::{ScopedIntervalsService, TestIntervalsService},
};

#[tokio::test]
async fn list_events_requires_authentication() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_events_returns_events_for_authenticated_user() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(
            11,
            "VO2 Session",
            Some("- 10min 55%\n- 5x3min 120%".to_string()),
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    let event = &body.as_array().unwrap()[0];
    assert_eq!(event.get("id").unwrap().as_i64().unwrap(), 11);
    assert_eq!(
        event
            .get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        event
            .get("eventDefinition")
            .unwrap()
            .get("rawWorkoutDoc")
            .unwrap()
            .as_str()
            .unwrap(),
        "- 10min 55%\n- 5x3min 120%"
    );
    assert!(event.get("actualWorkout").unwrap().is_null());
}

#[tokio::test]
async fn list_events_returns_422_when_credentials_not_configured() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::CredentialsNotConfigured),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn list_events_are_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    101,
                    "User One Workout",
                    Some("- 1x10min 90%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    202,
                    "User Two Workout",
                    Some("- 4x4min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response_user_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let response_user_2 = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body_user_1: Value = get_json(response_user_1).await;
    let body_user_2: Value = get_json(response_user_2).await;

    assert_eq!(body_user_1.as_array().unwrap().len(), 1);
    assert_eq!(body_user_2.as_array().unwrap().len(), 1);
    assert_eq!(
        body_user_1.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(101)
    );
    assert_eq!(
        body_user_2.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(202)
    );
}

#[tokio::test]
async fn list_events_rejects_invalid_date_query() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=20260301&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_events_rejects_impossible_calendar_date_query() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-02-31&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
