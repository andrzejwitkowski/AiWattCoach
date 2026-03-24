use std::{
    cell::RefCell,
    fs,
    future::Future,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{build_app_with_frontend_dist, AppState, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderValue, Method, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
const HTML_CONTENT_TYPE: &str = "text/html";
static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
static TRACE_CAPTURE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static TEST_TRACING_INIT: OnceLock<()> = OnceLock::new();
const DOCUMENT_ACCEPT: &str = "text/html,application/xhtml+xml";
const DOCUMENT_DEST: &str = "document";

thread_local! {
    static ACTIVE_LOG_BUFFER: RefCell<Option<SharedLogBuffer>> = const { RefCell::new(None) };
}

#[tokio::test]
async fn health_check_returns_service_status() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
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

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
}

#[tokio::test]
async fn readiness_returns_service_unavailable_without_mongo() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "degraded");
    assert_eq!(payload["reason"], "mongo_unreachable");
}

#[tokio::test]
async fn root_serves_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_html_response(response, fixture.index_html()).await;
}

#[tokio::test]
async fn built_frontend_fixture_serves_spa_at_root_while_health_stays_json() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let root_response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_html_response(root_response, fixture.index_html()).await;

    let health_response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(health_response.status(), StatusCode::OK);

    let content_type = health_response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("health response should include a content type");
    assert!(
        content_type.starts_with("application/json"),
        "expected JSON content type, got {content_type}"
    );

    let body = to_bytes(health_response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["service"], "AiWattCoach");
}

#[tokio::test]
async fn unknown_non_api_route_serves_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/settings")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_html_response(response, fixture.index_html()).await;
}

#[tokio::test]
async fn non_api_route_serves_spa_html_with_html_accept_and_no_fetch_metadata() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/settings")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_html_response(response, fixture.index_html()).await;
}

#[tokio::test]
async fn nested_non_api_route_serves_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/settings/profile")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_html_response(response, fixture.index_html()).await;
}

#[tokio::test]
async fn dotted_non_api_route_serves_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/jane.doe")
                .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
                .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_html_response(response, fixture.index_html()).await;
}

#[tokio::test]
async fn existing_normal_static_asset_is_served_directly() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_static_response(response, fixture.app_js(), "text/javascript").await;
}

#[tokio::test]
async fn existing_extensionless_static_asset_is_served_directly() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/no-extension-file")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_static_response(
        response,
        fixture.no_extension_file(),
        "application/octet-stream",
    )
    .await;
}

#[tokio::test]
async fn missing_asset_path_stays_not_found_and_non_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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

#[tokio::test]
async fn document_fetch_metadata_without_html_accept_does_not_fall_back() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for unknown API route: {content_type}"
    );
}

#[tokio::test]
async fn bare_api_route_does_not_fall_back_to_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
        .oneshot(Request::builder().uri("/api").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for bare API route: {content_type}"
    );
}

#[tokio::test]
async fn post_to_spa_route_does_not_fall_back_to_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for non-GET SPA route: {content_type}"
    );
}

#[tokio::test]
async fn post_api_route_does_not_fall_back_to_spa_html() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let response = app
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

#[tokio::test(flavor = "current_thread")]
async fn health_check_with_traceparent_logs_matching_trace_id() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );
    let trace_id = "0af7651916cd43dd8448eb211c80319c";

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/health")
                .header("traceparent", format!("00-{trace_id}-b7ad6b7169203331-01"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        logs.contains(trace_id),
        "expected logs to include propagated trace id {trace_id}, got: {logs}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn not_found_api_route_emits_warn_classification_log() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        logs.contains("\"level\":\"WARN\"") && logs.contains("\"status\":404"),
        "expected warn log for 404 response, got: {logs}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_check_emits_error_classification_log_for_service_unavailable() {
    let settings = unreachable_mongo_settings();
    let fixture = frontend_fixture();
    let app = build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    );

    let (response, logs) = capture_tracing_logs(|| async move {
        app.oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        logs.contains("\"level\":\"ERROR\"") && logs.contains("\"status\":503"),
        "expected error log for 503 response, got: {logs}"
    );
}

async fn assert_html_response(response: axum::response::Response, expected_html: &str) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("response should include a content type");
    assert!(
        content_type.starts_with(HTML_CONTENT_TYPE),
        "expected HTML content type, got {content_type}"
    );

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_text, expected_html);
}

async fn assert_head_html_response(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("response should include a content type");
    assert!(
        content_type.starts_with(HTML_CONTENT_TYPE),
        "expected HTML content type, got {content_type}"
    );

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert!(body.is_empty());
}

async fn assert_static_response(
    response: axum::response::Response,
    expected_body: &str,
    expected_content_type: &str,
) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("response should include a content type");
    assert!(
        content_type.starts_with(expected_content_type),
        "expected content type starting with {expected_content_type}, got {content_type}"
    );

    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_text, expected_body);
}

async fn assert_not_found_non_html_response(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for not found response: {content_type}"
    );
}

async fn test_mongo_client(uri: &str) -> mongodb::Client {
    mongodb::Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}

fn unreachable_mongo_settings() -> Settings {
    let mut settings = Settings::test_defaults();
    settings.mongo.uri = "mongodb://unresolvable.invalid:27017".to_string();
    settings
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FRONTEND_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-spa-fixture-{}-{unique}-{counter}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir).unwrap();
    fs::create_dir_all(dist_dir.join("assets")).unwrap();

    let index_html = "<!doctype html><html><head><title>Test SPA</title></head><body><div id=\"root\">fixture</div></body></html>".to_string();
    let app_js = "console.log('fixture asset');".to_string();
    let no_extension_file = "fixture file without an extension".to_string();
    fs::write(dist_dir.join("index.html"), &index_html).unwrap();
    fs::write(dist_dir.join("assets").join("app.js"), &app_js).unwrap();
    fs::write(dist_dir.join("no-extension-file"), &no_extension_file).unwrap();

    FrontendFixture {
        root,
        index_html,
        app_js,
        no_extension_file,
    }
}

struct FrontendFixture {
    root: PathBuf,
    index_html: String,
    app_js: String,
    no_extension_file: String,
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }

    fn index_html(&self) -> &str {
        &self.index_html
    }

    fn app_js(&self) -> &str {
        &self.app_js
    }

    fn no_extension_file(&self) -> &str {
        &self.no_extension_file
    }
}

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone, Default)]
struct SharedLogBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedLogBuffer {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("log buffer mutex poisoned").clone())
            .expect("log buffer contained invalid utf-8")
    }
}

impl Write for SharedLogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("log buffer mutex poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
    type Writer = SharedLogBuffer;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[derive(Clone, Default)]
struct ThreadLocalLogRouter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ThreadLocalLogRouter {
    type Writer = ThreadLocalLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ThreadLocalLogWriter
    }
}

struct ThreadLocalLogWriter;

impl Write for ThreadLocalLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        ACTIVE_LOG_BUFFER.with(|slot| {
            if let Some(buffer) = slot.borrow().as_ref() {
                let mut buffer = buffer.clone();
                buffer.write(buf)
            } else {
                Ok(buf.len())
            }
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct ActiveLogBufferGuard;

impl ActiveLogBufferGuard {
    fn install(buffer: SharedLogBuffer) -> Self {
        ACTIVE_LOG_BUFFER.with(|slot| {
            *slot.borrow_mut() = Some(buffer);
        });

        Self
    }
}

impl Drop for ActiveLogBufferGuard {
    fn drop(&mut self) {
        ACTIVE_LOG_BUFFER.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

async fn capture_tracing_logs<F, Fut, T>(run: F) -> (T, String)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let _capture_guard = TRACE_CAPTURE_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    init_test_tracing_subscriber();
    let logs = SharedLogBuffer::default();
    let _active_buffer = ActiveLogBufferGuard::install(logs.clone());
    let output = run().await;

    (output, logs.contents())
}

fn init_test_tracing_subscriber() {
    TEST_TRACING_INIT.get_or_init(|| {
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(ThreadLocalLogRouter)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("test tracing subscriber should install once");
    });
}
