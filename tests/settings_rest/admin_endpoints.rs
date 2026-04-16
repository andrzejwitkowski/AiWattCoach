use aiwattcoach::domain::identity::Role;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    session_cookie, settings_test_app, settings_test_app_with_completed_workout_service,
    TestCompletedWorkoutAdminService, TestIdentityServiceWithSession, TestSettingsService,
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

#[tokio::test]
async fn admin_can_backfill_completed_workout_details() {
    let service = TestCompletedWorkoutAdminService::default();
    let app = settings_test_app_with_completed_workout_service(
        TestIdentityServiceWithSession {
            roles: vec![Role::User, Role::Admin],
            ..Default::default()
        },
        TestSettingsService::default(),
        service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/completed-workouts/user-999/backfill-details?oldest=2026-04-01&newest=2026-04-16")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        service.calls(),
        vec![(
            "user-999".to_string(),
            "2026-04-01".to_string(),
            "2026-04-16".to_string(),
        )]
    );
}

#[tokio::test]
async fn non_admin_cannot_backfill_completed_workout_details() {
    let service = TestCompletedWorkoutAdminService::default();
    let app = settings_test_app_with_completed_workout_service(
        TestIdentityServiceWithSession {
            roles: vec![Role::User],
            ..Default::default()
        },
        TestSettingsService::default(),
        service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/completed-workouts/user-999/backfill-details?oldest=2026-04-01&newest=2026-04-16")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(service.calls().is_empty());
}

#[tokio::test]
async fn admin_backfill_completed_workout_details_rejects_invalid_date_range() {
    let service = TestCompletedWorkoutAdminService::default();
    let app = settings_test_app_with_completed_workout_service(
        TestIdentityServiceWithSession {
            roles: vec![Role::User, Role::Admin],
            ..Default::default()
        },
        TestSettingsService::default(),
        service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/completed-workouts/user-999/backfill-details?oldest=2026-04-99&newest=2026-04-16")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(service.calls().is_empty());
}
