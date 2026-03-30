use aiwattcoach::domain::identity::Role;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    session_cookie, settings_test_app, TestIdentityServiceWithSession, TestSettingsService,
};

#[tokio::test]
async fn admin_can_view_any_user_settings() {
    let app = settings_test_app(
        TestIdentityServiceWithSession {
            roles: vec![Role::User, Role::Admin],
            ..Default::default()
        },
        TestSettingsService::default(),
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

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn non_admin_cannot_view_other_user_settings() {
    let app = settings_test_app(
        TestIdentityServiceWithSession {
            roles: vec![Role::User],
            ..Default::default()
        },
        TestSettingsService::default(),
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

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
