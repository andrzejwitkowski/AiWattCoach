use aiwattcoach::domain::identity::IdentityError;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{auth_test_app, auth_test_app_without_identity, TestIdentityService};

#[tokio::test(flavor = "current_thread")]
async fn google_callback_returns_bad_request_for_invalid_login_state() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::InvalidLoginState),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_returns_service_unavailable_for_provider_failures() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::External("google timeout".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_returns_unauthorized_for_invalid_dev_auth_code() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::Unauthenticated),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=bad-dev-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn me_returns_service_unavailable_when_identity_backend_errors() {
    let app = auth_test_app(TestIdentityService {
        current_user_error: Some(IdentityError::Repository("mongo down".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/me")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test(flavor = "current_thread")]
async fn logout_returns_service_unavailable_when_session_invalidation_fails() {
    let app = auth_test_app(TestIdentityService {
        logout_error: Some(IdentityError::Repository("mongo down".to_string())),
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test(flavor = "current_thread")]
async fn logout_returns_service_unavailable_and_clears_cookie_without_identity_service() {
    let app = auth_test_app_without_identity().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test(flavor = "current_thread")]
async fn admin_system_info_returns_service_unavailable_for_backend_errors() {
    let app = auth_test_app(TestIdentityService {
        require_admin_error: Some(IdentityError::Repository("mongo down".to_string())),
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

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
