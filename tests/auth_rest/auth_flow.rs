use aiwattcoach::Settings;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use std::collections::BTreeMap;
use tower::util::ServiceExt;

use crate::shared::{auth_test_app, auth_test_app_with_custom_settings, TestIdentityService};

#[tokio::test]
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

#[tokio::test]
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

#[tokio::test]
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

#[tokio::test]
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
