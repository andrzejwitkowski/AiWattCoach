use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, GeminiSupervisorWebhookOutcome, TrainingPlanSupervisorDecision,
        TrainingPlanSupervisorOperation, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorStatus, TrainingPlanSupervisorWebhookUseCases,
    },
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    auth_test_app_with_gemini_supervisor_webhook, TestIdentityService, RESPONSE_LIMIT_BYTES,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct GeminiWebhookCall {
    worker_operation_key: String,
    provided_webhook_token: String,
    event_type: String,
    batch_name: String,
}

#[derive(Clone)]
struct TestGeminiSupervisorWebhookService {
    result: Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>,
    calls: Arc<Mutex<Vec<GeminiWebhookCall>>>,
}

impl TestGeminiSupervisorWebhookService {
    fn accepting() -> Self {
        let pending = TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_000,
        );
        let accepted = pending
            .complete_review(
                TrainingPlanSupervisorReview {
                    decision: TrainingPlanSupervisorDecision::Accept,
                    reason: "plan is ready".to_string(),
                    plan: None,
                },
                1_700_000_100,
            )
            .expect("accepted review should be valid");

        Self {
            result: Ok(GeminiSupervisorWebhookOutcome::Accepted(accepted)),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn ignored() -> Self {
        Self {
            result: Ok(GeminiSupervisorWebhookOutcome::Ignored),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn invalid_token() -> Self {
        Self {
            result: Err(TrainingPlanError::Validation(
                "Gemini supervisor webhook token is invalid".to_string(),
            )),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn not_configured() -> Self {
        Self {
            result: Err(TrainingPlanError::Unavailable(
                "Gemini supervisor webhook is not configured".to_string(),
            )),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn invalid_payload() -> Self {
        Self {
            result: Err(TrainingPlanError::Validation(
                "Gemini supervisor webhook payload is invalid".to_string(),
            )),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<GeminiWebhookCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl TrainingPlanSupervisorWebhookUseCases for TestGeminiSupervisorWebhookService {
    fn receive_gemini_batch_webhook(
        &self,
        worker_operation_key: &str,
        provided_webhook_token: &str,
        event_type: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<GeminiSupervisorWebhookOutcome, TrainingPlanError>> {
        self.calls.lock().unwrap().push(GeminiWebhookCall {
            worker_operation_key: worker_operation_key.to_string(),
            provided_webhook_token: provided_webhook_token.to_string(),
            event_type: event_type.to_string(),
            batch_name: batch_name.to_string(),
        });
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn gemini_supervisor_webhook_returns_ok_when_payload_is_accepted() {
    let service = TestGeminiSupervisorWebhookService::accepting();
    let app = auth_test_app_with_gemini_supervisor_webhook(
        TestIdentityService::default(),
        service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/training-plan-supervisor/gemini/webhook/worker-op-1/secret-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "type": "batch.succeeded",
                        "data": { "id": "batches/batch-1" }
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
        service.calls(),
        vec![GeminiWebhookCall {
            worker_operation_key: "worker-op-1".to_string(),
            provided_webhook_token: "secret-token".to_string(),
            event_type: "batch.succeeded".to_string(),
            batch_name: "batches/batch-1".to_string(),
        }]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn gemini_supervisor_webhook_returns_ok_when_event_is_ignored() {
    let service = TestGeminiSupervisorWebhookService::ignored();
    let app = auth_test_app_with_gemini_supervisor_webhook(
        TestIdentityService::default(),
        service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/training-plan-supervisor/gemini/webhook/worker-op-1/secret-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "type": "batch.failed",
                        "data": { "id": "batches/batch-1" }
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
async fn gemini_supervisor_webhook_returns_401_for_invalid_token() {
    let app = auth_test_app_with_gemini_supervisor_webhook(
        TestIdentityService::default(),
        TestGeminiSupervisorWebhookService::invalid_token(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/training-plan-supervisor/gemini/webhook/worker-op-1/wrong-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "type": "batch.succeeded",
                        "data": { "id": "batches/batch-1" }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "current_thread")]
async fn gemini_supervisor_webhook_returns_503_when_not_configured() {
    let app = auth_test_app_with_gemini_supervisor_webhook(
        TestIdentityService::default(),
        TestGeminiSupervisorWebhookService::not_configured(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/training-plan-supervisor/gemini/webhook/worker-op-1/secret-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "type": "batch.succeeded",
                        "data": { "id": "batches/batch-1" }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test(flavor = "current_thread")]
async fn gemini_supervisor_webhook_returns_400_for_validation_error() {
    let app = auth_test_app_with_gemini_supervisor_webhook(
        TestIdentityService::default(),
        TestGeminiSupervisorWebhookService::invalid_payload(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/training-plan-supervisor/gemini/webhook/worker-op-1/secret-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{
                        "type": "batch.succeeded",
                        "data": { "id": "batches/batch-1" }
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn accepted_test_operation_is_terminal() {
    let operation = TrainingPlanSupervisorOperation::pending(
        "worker-op-1".to_string(),
        "user-1".to_string(),
        1_700_000_000,
        "gemini-2.5-pro".to_string(),
        1_700_000_000,
    )
    .complete_review(
        TrainingPlanSupervisorReview {
            decision: TrainingPlanSupervisorDecision::Accept,
            reason: "plan is ready".to_string(),
            plan: None,
        },
        1_700_000_100,
    )
    .unwrap();

    assert_eq!(operation.status, TrainingPlanSupervisorStatus::Accepted);
}
