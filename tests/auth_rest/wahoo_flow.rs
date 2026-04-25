use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{auth_test_app_with_wahoo, TestIdentityService, TestWahooService};

#[tokio::test(flavor = "current_thread")]
async fn wahoo_start_redirects_to_provider_and_forwards_return_to() {
    let wahoo_service = TestWahooService::default();
    let captured = wahoo_service.last_begin_input.clone();
    let app = auth_test_app_with_wahoo(TestIdentityService::default(), wahoo_service).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/wahoo/start?returnTo=%2Fsettings%3Ftab%3Dintegrations")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "https://api.wahooligan.com/oauth/authorize?state=wahoo-state-1"
    );
    assert_eq!(
        captured.lock().unwrap().clone(),
        Some((
            "user-1".to_string(),
            Some("/settings?tab=integrations".to_string()),
        ))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_callback_redirects_back_to_settings() {
    let wahoo_service = TestWahooService::default();
    let captured = wahoo_service.last_finish_input.clone();
    let app = auth_test_app_with_wahoo(TestIdentityService::default(), wahoo_service).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/wahoo/callback?state=wahoo-state-1&code=oauth-code")
                .header(header::COOKIE, "aiwattcoach_session=session-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/settings?connected=wahoo"
    );
    assert_eq!(
        captured.lock().unwrap().clone(),
        Some((
            "user-1".to_string(),
            "wahoo-state-1".to_string(),
            "oauth-code".to_string(),
        ))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_callback_requires_authenticated_user() {
    let app =
        auth_test_app_with_wahoo(TestIdentityService::default(), TestWahooService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/wahoo/callback?state=wahoo-state-1&code=oauth-code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
