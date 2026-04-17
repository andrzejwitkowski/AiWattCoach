use aiwattcoach::{domain::identity::IdentityError, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use std::collections::BTreeMap;
use tower::util::ServiceExt;

use crate::shared::{
    auth_test_app, auth_test_app_with_custom_settings, auth_test_app_with_limited_whitelist_rate,
    TestIdentityService, RESPONSE_LIMIT_BYTES,
};

#[tokio::test(flavor = "current_thread")]
async fn google_start_redirects_to_provider() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/start?returnTo=%2Fsettings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn google_start_drops_unsafe_return_to_values() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/start?returnTo=https%3A%2F%2Fevil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_sets_cookie_and_redirects_into_calendar() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/calendar"
    );

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session=session-1"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_redirects_to_landing_without_cookie_when_pending_approval() {
    let app = auth_test_app(TestIdentityService {
        callback_error: Some(IdentityError::PendingApproval),
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

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/?auth=pending-approval"
    );
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_redirects_to_pending_approval_result_without_cookie() {
    let app = auth_test_app(TestIdentityService {
        pending_approval_redirect_to: Some(
            "/?auth=pending-approval&returnTo=%2Fsettings%3Ftab%3Dsecurity".to_string(),
        ),
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

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/?auth=pending-approval&returnTo=%2Fsettings%3Ftab%3Dsecurity"
    );
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_sets_none_same_site_cookie_for_cross_site_mode() {
    let settings = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3002".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
        (
            "GOOGLE_OAUTH_CLIENT_ID".to_string(),
            "client-id.apps.googleusercontent.com".to_string(),
        ),
        (
            "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
            "super-secret".to_string(),
        ),
        (
            "GOOGLE_OAUTH_REDIRECT_URL".to_string(),
            "http://localhost:3002/api/auth/google/callback".to_string(),
        ),
        (
            "SESSION_COOKIE_NAME".to_string(),
            "aiwattcoach_session".to_string(),
        ),
        ("SESSION_COOKIE_SAME_SITE".to_string(), "none".to_string()),
        ("SESSION_TTL_HOURS".to_string(), "24".to_string()),
        ("SESSION_COOKIE_SECURE".to_string(), "true".to_string()),
    ]))
    .unwrap();
    let app = auth_test_app_with_custom_settings(settings, TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("SameSite=None"));
    assert!(cookie.contains("Secure"));
}

#[tokio::test(flavor = "current_thread")]
async fn frontend_fallback_serves_index_from_kept_fixture() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/calendar")
                .header(header::ACCEPT, "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "current_thread")]
async fn google_start_forwards_return_to_to_identity_service() {
    let service = TestIdentityService::default();
    let captured = service.last_return_to.clone();
    let app = auth_test_app(service).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/start?returnTo=%2Fsettings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(captured.lock().unwrap().as_deref(), Some("/settings"));
}

#[tokio::test(flavor = "current_thread")]
async fn google_callback_forwards_state_and_code_to_identity_service() {
    let service = TestIdentityService::default();
    let captured = service.last_callback_input.clone();
    let app = auth_test_app(service).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/google/callback?state=state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        captured.lock().unwrap().clone(),
        Some(("state-1".to_string(), "oauth-code".to_string()))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn join_whitelist_returns_success_and_forwards_email() {
    let service = TestIdentityService::default();
    let captured = service.last_join_whitelist_email.clone();
    let app = auth_test_app(service).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"email":"athlete@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["success"], true);
    assert_eq!(
        captured.lock().unwrap().as_deref(),
        Some("athlete@example.com")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn join_whitelist_rejects_invalid_email_payload() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"email":"not-an-email"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "current_thread")]
async fn join_whitelist_rejects_email_with_multiple_at_symbols() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"email":"athlete@@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "current_thread")]
async fn join_whitelist_surfaces_service_errors_as_service_unavailable() {
    let service = TestIdentityService {
        join_whitelist_error: Some(IdentityError::External(
            "whitelist repository down".to_string(),
        )),
        ..Default::default()
    };
    let captured = service.last_join_whitelist_email.clone();
    let app = auth_test_app(service).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"email":"athlete@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        captured.lock().unwrap().as_deref(),
        Some("athlete@example.com")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn join_whitelist_rate_limits_repeated_requests_from_same_ip() {
    let service = TestIdentityService::default();
    let captured = service.last_join_whitelist_email.clone();
    let app = auth_test_app_with_limited_whitelist_rate(service, 1).await;

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "203.0.113.7")
                .body(Body::from(r#"{"email":"athlete@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let second_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/whitelist")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-forwarded-for", "203.0.113.7")
                .body(Body::from(r#"{"email":"second@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        captured.lock().unwrap().as_deref(),
        Some("athlete@example.com")
    );
}
