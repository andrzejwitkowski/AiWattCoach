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
    http::{header, Request, StatusCode},
};
use mongodb::Client;
use serde_json::{json, Value};
use tower::util::ServiceExt;

const RESPONSE_LIMIT_BYTES: usize = 16 * 1024;
static FRONTEND_FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
static TRACE_CAPTURE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static TEST_TRACING_INIT: OnceLock<()> = OnceLock::new();

thread_local! {
    static ACTIVE_LOG_BUFFER: RefCell<Option<SharedLogBuffer>> = const { RefCell::new(None) };
}

#[tokio::test(flavor = "current_thread")]
async fn valid_info_warn_and_error_payloads_are_accepted() {
    let app = logs_test_app().await;

    for (level, expected_level) in [("info", "INFO"), ("warn", "WARN"), ("error", "ERROR")] {
        let message = format!("{level} message from client");
        let (response, logs) = capture_tracing_logs(|| async {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/logs")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "level": level,
                                "message": message,
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap()
        })
        .await;

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(
            response_json(response).await,
            json!({ "status": "accepted" })
        );
        assert!(
            logs.contains(&format!("\"client_log_level\":\"{level}\"")),
            "logs were: {logs}"
        );
        assert!(
            logs.contains(&format!("\"client_message\":\"{message}\"")),
            "logs were: {logs}"
        );
        assert!(
            logs.contains(&format!("\"level\":\"{expected_level}\"")),
            "logs were: {logs}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_level_is_rejected() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "debug",
                        "message": "unsupported",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({ "error": "unsupported_level" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_message_is_rejected() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "x".repeat(10_001),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await,
        json!({ "error": "message_too_long" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn near_limit_valid_payload_is_still_accepted() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "x".repeat(9_900),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response_json(response).await,
        json!({ "status": "accepted" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn escaped_near_limit_valid_payload_is_still_accepted() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "\"".repeat(10_000),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response_json(response).await,
        json!({ "status": "accepted" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_request_body_is_rejected_before_json_parsing() {
    let app = logs_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "level": "info",
                        "message": "\"".repeat(40_000),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

async fn logs_test_app() -> axum::Router {
    let settings = Settings::test_defaults();
    let fixture = frontend_fixture();

    build_app_with_frontend_dist(
        AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        ),
        fixture.dist_dir(),
    )
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .expect("response body to be collected");
    serde_json::from_slice(&body).expect("response to contain valid JSON")
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

struct FrontendFixture {
    root: PathBuf,
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FRONTEND_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-logs-spa-fixture-{}-{unique}-{counter}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir).unwrap();
    fs::write(
        dist_dir.join("index.html"),
        "<!doctype html><html><body><div id=\"root\">fixture</div></body></html>",
    )
    .unwrap();

    FrontendFixture { root }
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }
}

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}
