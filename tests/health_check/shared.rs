use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{build_app_with_frontend_dist, AppState, Settings};
use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderValue, Request, StatusCode},
};

pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
pub(crate) const HTML_CONTENT_TYPE: &str = "text/html";
pub(crate) const DOCUMENT_ACCEPT: &str = "text/html,application/xhtml+xml";
pub(crate) const DOCUMENT_DEST: &str = "document";

static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct HealthTestApp {
    pub(crate) app: axum::Router,
    fixture: FrontendFixture,
}

impl HealthTestApp {
    pub(crate) fn index_html(&self) -> &str {
        self.fixture.index_html()
    }

    pub(crate) fn app_js(&self) -> &str {
        self.fixture.app_js()
    }

    pub(crate) fn no_extension_file(&self) -> &str {
        self.fixture.no_extension_file()
    }
}

pub(crate) async fn health_test_app() -> HealthTestApp {
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

    HealthTestApp { app, fixture }
}

pub(crate) async fn assert_html_response(response: axum::response::Response, expected_html: &str) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
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

pub(crate) async fn assert_head_html_response(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
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

pub(crate) async fn assert_static_response(
    response: axum::response::Response,
    expected_body: &str,
    expected_content_type: &str,
) {
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
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

pub(crate) async fn assert_not_found_non_html_response(response: axum::response::Response) {
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with(HTML_CONTENT_TYPE),
        "unexpected HTML content type for not found response: {content_type}"
    );
}

pub(crate) fn html_navigation_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::ACCEPT, HeaderValue::from_static(DOCUMENT_ACCEPT))
        .header("Sec-Fetch-Dest", HeaderValue::from_static(DOCUMENT_DEST))
        .body(Body::empty())
        .unwrap()
}

pub(crate) fn assert_log_entry_contains(logs: &str, expected_fragments: &[&str]) {
    let matched = logs.lines().any(|line| {
        expected_fragments
            .iter()
            .all(|fragment| line.contains(fragment))
    });

    assert!(
        matched,
        "expected one log entry to contain {expected_fragments:?}, logs were: {logs}"
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
