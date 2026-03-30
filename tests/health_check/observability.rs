use axum::{body::Body, http::Request, http::StatusCode};
use tower::util::ServiceExt;

use crate::{
    shared::{assert_log_entry_contains, health_test_app},
    tracing_capture::capture_tracing_logs,
};

#[tokio::test(flavor = "current_thread")]
async fn health_check_with_traceparent_logs_matching_trace_id() {
    let test_app = health_test_app().await;
    let trace_id = "0af7651916cd43dd8448eb211c80319c";

    let (response, logs) = capture_tracing_logs(|| async move {
        test_app
            .app
            .oneshot(
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
    let test_app = health_test_app().await;

    let (response, logs) = capture_tracing_logs(|| async move {
        test_app
            .app
            .oneshot(
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
    assert_log_entry_contains(&logs, &["\"level\":\"WARN\"", "\"status\":404"]);
}

#[tokio::test(flavor = "current_thread")]
async fn readiness_check_emits_error_classification_log_for_service_unavailable() {
    let test_app = health_test_app().await;

    let (response, logs) = capture_tracing_logs(|| async move {
        test_app
            .app
            .oneshot(
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
    assert_log_entry_contains(&logs, &["\"level\":\"ERROR\"", "\"status\":503"]);
}

#[tokio::test(flavor = "current_thread")]
async fn health_check_without_traceparent_logs_generated_trace_id() {
    let test_app = health_test_app().await;

    let (response, logs) = capture_tracing_logs(|| async move {
        test_app
            .app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        logs.lines().any(|line| {
            line.contains("\"trace_id\":\"")
                && !line.contains("\"trace_id\":\"00000000000000000000000000000000\"")
        }),
        "expected logs to include a generated non-zero trace id, got: {logs}"
    );
}
