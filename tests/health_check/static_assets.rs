use axum::{
    body::Body,
    http::{header, HeaderValue, Request},
};
use tower::util::ServiceExt;

use crate::shared::{
    assert_not_found_non_html_response, assert_static_response, health_test_app, DOCUMENT_ACCEPT,
};

#[tokio::test]
async fn existing_normal_static_asset_is_served_directly() {
    let test_app = health_test_app().await;
    let expected_body = test_app.app_js().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_static_response(response, &expected_body, "text/javascript").await;
}

#[tokio::test]
async fn existing_extensionless_static_asset_is_served_directly() {
    let test_app = health_test_app().await;
    let expected_body = test_app.no_extension_file().to_string();

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/no-extension-file")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_static_response(response, &expected_body, "application/octet-stream").await;
}

#[tokio::test]
async fn missing_asset_path_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/assets/missing.js")
                .header(header::ACCEPT, HeaderValue::from_static("*/*"))
                .header("Sec-Fetch-Dest", HeaderValue::from_static("script"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn missing_root_level_asset_path_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/robots.txt")
                .header(header::ACCEPT, HeaderValue::from_static("text/plain,*/*"))
                .header("Sec-Fetch-Dest", HeaderValue::from_static("empty"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn missing_html_file_path_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/missing-page.html")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn missing_apple_app_site_association_path_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/apple-app-site-association")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn missing_root_level_file_like_path_with_html_accept_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/robots.txt")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}

#[tokio::test]
async fn missing_well_known_file_like_path_with_html_accept_stays_not_found_and_non_html() {
    let test_app = health_test_app().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .uri("/.well-known/assetlinks.json")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_not_found_non_html_response(response).await;
}
