use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{auth_test_app, TestIdentityService, RESPONSE_LIMIT_BYTES};

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_requires_authentication() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_rejects_non_admin_user() {
    let app = auth_test_app(TestIdentityService {
        admin_cookie_role: aiwattcoach::domain::identity::Role::User,
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_rejects_stale_cookie_as_unauthorized() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=missing-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_returns_payload_for_admin() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/system-info")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
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

    assert_eq!(payload["appName"], "AiWattCoach");
    assert_eq!(payload["mongoDatabase"], "aiwattcoach");
}
