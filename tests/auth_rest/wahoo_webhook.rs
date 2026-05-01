use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use std::sync::Arc;
use tokio::sync::{Barrier, Notify};
use tower::util::ServiceExt;

use aiwattcoach::domain::wahoo::{
    BoxFuture, ManualWahooSyncResult, WahooWebhookAccepted, WahooWebhookError, WahooWebhookOutcome,
    WahooWebhookUseCases, WahooWorkout,
};

use crate::shared::{
    auth_test_app_with_wahoo_webhook, TestIdentityService, TestWahooWebhookService,
    WahooWebhookImportCall, RESPONSE_LIMIT_BYTES,
};

#[derive(Clone)]
struct BlockingWahooWebhookService {
    started: Arc<Barrier>,
    release: Arc<Notify>,
}

#[derive(Clone, Default)]
struct InvalidPayloadWahooWebhookService;

impl WahooWebhookUseCases for BlockingWahooWebhookService {
    fn import_webhook_workout(
        &self,
        _webhook_token: &str,
        _wahoo_user_id: i64,
        _workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>> {
        let started = self.started.clone();
        let release = self.release.clone();
        Box::pin(async move {
            started.wait().await;
            release.notified().await;
            Ok(WahooWebhookOutcome::Accepted(WahooWebhookAccepted {
                user_id: "user-1".to_string(),
                completed_workout_id: "wahoo-workout:42".to_string(),
            }))
        })
    }

    fn sync_completed_workouts_for_user(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<ManualWahooSyncResult, WahooWebhookError>> {
        Box::pin(async {
            Ok(ManualWahooSyncResult {
                scanned: 0,
                imported: 0,
                skipped: 0,
            })
        })
    }
}

impl WahooWebhookUseCases for InvalidPayloadWahooWebhookService {
    fn import_webhook_workout(
        &self,
        _webhook_token: &str,
        _wahoo_user_id: i64,
        _workout: WahooWorkout,
    ) -> BoxFuture<Result<WahooWebhookOutcome, WahooWebhookError>> {
        Box::pin(async {
            Err(WahooWebhookError::InvalidPayload(
                "Wahoo workout payload did not produce a valid import command".to_string(),
            ))
        })
    }

    fn sync_completed_workouts_for_user(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<ManualWahooSyncResult, WahooWebhookError>> {
        Box::pin(async {
            Ok(ManualWahooSyncResult {
                scanned: 0,
                imported: 0,
                skipped: 0,
            })
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_returns_ok_when_payload_is_accepted() {
    let service = TestWahooWebhookService::accepting();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "secret-token",
                        "user": { "id": 60462 },
                        "workout_summary": {
                            "id": 42,
                            "name": "Morning Ride",
                            "distance_meters": 20000.0,
                            "duration_total_seconds": 3600.0,
                            "duration_active_seconds": 3600.0,
                            "calories": 500.0,
                            "normalized_power_watts": 220.0,
                            "training_stress_score": 80.0,
                            "average_power_watts": 200.0,
                            "file": { "url": "https://example.test/42.fit" },
                            "manual": false,
                            "edited": false
                        },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z",
                            "minutes": 60,
                            "name": "Morning Ride",
                            "workout_type_id": 12
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), br#"{"accepted":true}"#);
    assert_eq!(
        service.import_calls(),
        vec![WahooWebhookImportCall {
            webhook_token: "secret-token".to_string(),
            wahoo_user_id: 60_462,
            workout_id: 42,
            starts: "2023-11-14T08:00:00Z".to_string(),
            has_workout_summary: true,
        }]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_returns_401_for_invalid_token() {
    let service = TestWahooWebhookService::unauthorized();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "wrong-token",
                        "user": { "id": 60462 },
                        "workout_summary": { "id": 42, "manual": false, "edited": false },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        service.import_calls(),
        vec![WahooWebhookImportCall {
            webhook_token: "wrong-token".to_string(),
            wahoo_user_id: 60_462,
            workout_id: 42,
            starts: "2023-11-14T08:00:00Z".to_string(),
            has_workout_summary: true,
        }]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_returns_503_when_webhook_token_not_configured() {
    let service = TestWahooWebhookService::not_configured();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "secret-token",
                        "user": { "id": 60462 },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_ignores_unknown_user_for_workout_summary() {
    let service = TestWahooWebhookService::ignored();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "secret-token",
                        "user": { "id": 999999 },
                        "workout_summary": {
                            "id": 42,
                            "manual": false,
                            "edited": false
                        },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), br#"{"accepted":false}"#);
    assert_eq!(service.import_calls().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_ignores_non_workout_summary_events() {
    let service = TestWahooWebhookService::accepting();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "device_status",
                        "webhook_token": "secret-token",
                        "user": { "id": 60462 },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), br#"{"accepted":false}"#);
    assert!(service.import_calls().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_ignores_non_summary_events_even_without_workout_shape() {
    let service = TestWahooWebhookService::accepting();
    let app = auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "device_status",
                        "unexpected": { "payload": true }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), br#"{"accepted":false}"#);
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_accepts_workout_summary_event_with_null_file_url() {
    let service = TestWahooWebhookService::accepting();
    let app =
        auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "secret-token",
                        "user": { "id": 60462 },
                        "workout_summary": {
                            "id": 42,
                            "file": { "url": null },
                            "manual": false,
                            "edited": false
                        },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), br#"{"accepted":true}"#);
    assert_eq!(service.import_calls().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_returns_400_for_workout_summary_event_without_summary_payload() {
    let app = auth_test_app_with_wahoo_webhook(
        TestIdentityService::default(),
        InvalidPayloadWahooWebhookService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wahoo/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "event_type": "workout_summary",
                        "webhook_token": "secret-token",
                        "user": { "id": 60462 },
                        "workout": {
                            "id": 42,
                            "starts": "2023-11-14T08:00:00Z"
                        }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "current_thread")]
async fn wahoo_webhook_waits_for_service_before_returning_ok() {
    let service = BlockingWahooWebhookService {
        started: Arc::new(Barrier::new(2)),
        release: Arc::new(Notify::new()),
    };
    let started = service.started.clone();
    let release = service.release.clone();
    let app = auth_test_app_with_wahoo_webhook(TestIdentityService::default(), service).await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/wahoo/webhook")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{
                "event_type": "workout_summary",
                "webhook_token": "secret-token",
                "user": { "id": 60462 },
                "workout_summary": {
                    "id": 42,
                    "manual": false,
                    "edited": false
                },
                "workout": {
                    "id": 42,
                    "starts": "2023-11-14T08:00:00Z"
                }
            }"#,
        ))
        .unwrap();

    let response_task = tokio::spawn(async move { app.oneshot(request).await.unwrap() });

    started.wait().await;
    assert!(!response_task.is_finished());

    release.notify_waiters();
    let response = response_task.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
