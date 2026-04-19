use aiwattcoach::domain::identity::Role;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    session_cookie, settings_test_app, settings_test_app_with_completed_workout_service,
    DetailBackfillCall, MetricsBackfillCall, MetricsBackfillRange,
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
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        service.detail_calls(),
        vec![DetailBackfillCall {
            user_id: "user-999".to_string(),
            oldest: "2026-04-01".to_string(),
            newest: "2026-04-16".to_string(),
        }]
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
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(service.detail_calls().is_empty());
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
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(service.detail_calls().is_empty());
}

#[tokio::test]
async fn admin_backfill_completed_workout_details_rejects_oldest_after_newest() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-details?oldest=2026-04-16&newest=2026-04-01")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(service.detail_calls().is_empty());
}

#[tokio::test]
async fn admin_backfill_completed_workout_details_returns_503_when_service_fails() {
    let service = TestCompletedWorkoutAdminService::failing("repository unavailable");
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
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(service.detail_calls().len(), 1);
}

#[tokio::test]
async fn admin_can_backfill_completed_workout_metrics_for_all_workouts() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-metrics")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        service.metric_calls(),
        vec![MetricsBackfillCall {
            user_id: "user-999".to_string(),
            range: MetricsBackfillRange {
                oldest: None,
                newest: None,
            },
        }]
    );
}

#[tokio::test]
async fn admin_can_backfill_completed_workout_metrics_for_date_range() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-metrics?oldest=2026-04-01&newest=2026-04-16")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        service.metric_calls(),
        vec![MetricsBackfillCall {
            user_id: "user-999".to_string(),
            range: MetricsBackfillRange {
                oldest: Some("2026-04-01".to_string()),
                newest: Some("2026-04-16".to_string()),
            },
        }]
    );
}

#[tokio::test]
async fn admin_backfill_completed_workout_metrics_rejects_partial_date_range() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-metrics?oldest=2026-04-01")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(service.metric_calls().is_empty());
}

#[tokio::test]
async fn non_admin_cannot_backfill_completed_workout_metrics() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-metrics")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(service.metric_calls().is_empty());
}

#[tokio::test]
async fn admin_backfill_completed_workout_metrics_rejects_cross_origin_request() {
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
                .uri("/api/admin/completed-workouts/user-999/backfill-metrics")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(service.metric_calls().is_empty());
}

#[tokio::test]
async fn admin_backfill_completed_workout_details_rejects_cross_origin_request() {
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
                .header(header::HOST, "localhost")
                .header(header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(service.detail_calls().is_empty());
}
