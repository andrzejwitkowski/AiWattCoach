use axum::{
    body::Body,
    http::{header, HeaderValue, Method, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    assert_head_html_response, assert_html_response, assert_not_found_non_html_response,
    health_test_app, html_navigation_request, DOCUMENT_ACCEPT, DOCUMENT_DEST, HTML_CONTENT_TYPE,
};

#[tokio::test]
async fn unknown_non_api_route_serves_spa_html() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(html_navigation_request("/settings"))
        .await
        .unwrap();

    assert_html_response(response, &expected_html).await;
}

#[tokio::test]
async fn non_api_route_serves_spa_html_with_html_accept_and_no_fetch_metadata() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/settings")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_html_response(response, &expected_html).await;
}

#[tokio::test]
async fn nested_non_api_route_serves_spa_html() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(html_navigation_request("/settings/profile"))
        .await
        .unwrap();

    assert_html_response(response, &expected_html).await;
}

#[tokio::test]
async fn dotted_non_api_route_serves_spa_html() {
    let test_app = health_test_app().await;
    let expected_html = test_app.index_html().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(html_navigation_request("/users/jane.doe"))
        .await
        .unwrap();

    assert_html_response(response, &expected_html).await;
}

#[tokio::test]
async fn document_fetch_metadata_without_html_accept_does_not_fall_back() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/settings")
                .header(header::ACCEPT, HeaderValue::from_static("application/json"))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn head_root_serves_spa_html_headers() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_head_html_response(response).await;
}

#[tokio::test]
async fn head_non_api_route_serves_spa_html_headers() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/settings")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_head_html_response(response).await;
}

#[tokio::test]
async fn head_dotted_non_api_route_serves_spa_html_headers() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/users/jane.doe")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_head_html_response(response).await;
}

#[tokio::test]
async fn unknown_api_route_does_not_fall_back_to_spa_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/api/unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for unknown API route: {content_type}"
    );
}

#[tokio::test]
async fn bare_api_route_does_not_fall_back_to_spa_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(Request::builder().uri("/api").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for bare API route: {content_type}"
    );
}

#[tokio::test]
async fn post_to_spa_route_does_not_fall_back_to_spa_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for non-GET SPA route: {content_type}"
    );
}

#[tokio::test]
async fn post_api_route_does_not_fall_back_to_spa_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/unknown")
                .header(header::ACCEPT, HeaderValue::from_static("text/html"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}
