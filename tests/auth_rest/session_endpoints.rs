use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderValue, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{auth_test_app, TestIdentityService, RESPONSE_LIMIT_BYTES};

#[tokio::test]
async fn me_returns_unauthenticated_without_cookie() {
    let app = auth_test_app(TestIdentityService::default()).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/me")
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

    assert_eq!(payload["authenticated"], false);
}

#[tokio::test]
async fn me_returns_current_user_when_cookie_matches_session() {
    let app = auth_test_app(TestIdentityService::default()).await;

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

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["authenticated"], true);
    assert_eq!(payload["user"]["email"], "admin@example.com");
    assert_eq!(payload["user"]["roles"][0], "user");
    assert_eq!(payload["user"]["roles"][1], "admin");
}

#[tokio::test]
async fn me_reads_session_cookie_from_later_cookie_header() {
    let app = auth_test_app(TestIdentityService::default()).await;
    let mut request = Request::builder()
        .uri("/api/auth/me")
        .body(Body::empty())
        .unwrap();
    request
        .headers_mut()
        .append(header::COOKIE, HeaderValue::from_static("theme=midnight"));
    request.headers_mut().append(
        header::COOKIE,
        HeaderValue::from_static("aiwattcoach_session=session-1"),
    );

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["authenticated"], true);
    assert_eq!(payload["user"]["email"], "admin@example.com");
}

#[tokio::test]
async fn logout_clears_session_cookie() {
    let app = auth_test_app(TestIdentityService::default()).await;

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

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.contains("aiwattcoach_session="));
    assert!(cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn logout_forwards_session_id_to_identity_service() {
    let service = TestIdentityService::default();
    let captured = service.last_logout_session_id.clone();
    let app = auth_test_app(service).await;

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

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(captured.lock().unwrap().as_deref(), Some("session-1"));
}
