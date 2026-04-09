use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    existing_summary, InMemoryCoachReplyOperationRepository, InMemoryWorkoutSummaryRepository,
    TestAvailabilitySettingsService, TestClock, TestIdGenerator,
};
use crate::shared::{
    get_json, sample_summary, sample_summary_with_updated_at, session_cookie,
    workout_summary_test_app, workout_summary_test_app_with_settings,
    TestIdentityServiceWithSession, TestWorkoutSummaryService,
};
use aiwattcoach::domain::workout_summary::WorkoutSummaryService;

#[tokio::test]
async fn get_summary_requires_authentication() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries/workout-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_summary_returns_not_found_when_missing() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries/workout-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_summary_returns_created_summary() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("workoutId").unwrap().as_str().unwrap(),
        "workout-1"
    );
    assert!(body.get("messages").unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_summary_returns_existing_summary() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("workout-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries/workout-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("id").unwrap().as_str().unwrap(),
        "summary-workout-1"
    );
    assert_eq!(body.get("rpe").unwrap().as_u64().unwrap(), 6);
}

#[tokio::test]
async fn list_summaries_returns_batch_results() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![
            sample_summary_with_updated_at("workout-1", 1_700_000_050),
            sample_summary_with_updated_at("workout-2", 1_700_000_100),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries?workoutIds=workout-1,workout-2,workout-3")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    let summaries = body.as_array().unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[0].get("workoutId").unwrap().as_str().unwrap(),
        "workout-2"
    );
    assert_eq!(
        summaries[1].get("workoutId").unwrap().as_str().unwrap(),
        "workout-1"
    );
}

#[tokio::test]
async fn list_summaries_rejects_empty_workout_ids() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries?workoutIds=,,")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("error").and_then(Value::as_str),
        Some("workoutIds must contain at least one workout id")
    );
}

#[tokio::test]
async fn update_rpe_returns_updated_summary() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("workout-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/workout-summaries/workout-1/rpe")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"rpe":8}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(body.get("rpe").unwrap().as_u64().unwrap(), 8);
}

#[tokio::test]
async fn save_summary_marks_summary_as_saved() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("workout-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/state")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"saved":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("summary")
            .and_then(|summary| summary.get("savedAtEpochSeconds"))
            .and_then(Value::as_i64),
        Some(1_700_000_100)
    );
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("recapStatus"))
            .and_then(Value::as_str),
        Some("skipped")
    );
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("planStatus"))
            .and_then(Value::as_str),
        Some("skipped")
    );
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("messages"))
            .and_then(Value::as_array)
            .map(|messages| messages.len()),
        Some(0)
    );
}

#[tokio::test]
async fn save_summary_requires_rpe() {
    let mut summary = sample_summary("workout-1");
    summary.rpe = None;
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![summary]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/state")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"saved":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reopen_summary_clears_saved_flag() {
    let mut summary = sample_summary("workout-1");
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![summary]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/workout-summaries/workout-1/state")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"saved":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(body
        .get("summary")
        .and_then(|summary| summary.get("savedAtEpochSeconds"))
        .is_some_and(Value::is_null));
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("recapStatus"))
            .and_then(Value::as_str),
        Some("unchanged")
    );
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("planStatus"))
            .and_then(Value::as_str),
        Some("unchanged")
    );
    assert_eq!(
        body.get("workflow")
            .and_then(|workflow| workflow.get("messages"))
            .and_then(Value::as_array)
            .map(|messages| messages.len()),
        Some(0)
    );
}

#[tokio::test]
async fn send_message_returns_persisted_turn() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("workout-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/messages")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"Legs felt heavy today"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("userMessage")
            .unwrap()
            .get("role")
            .unwrap()
            .as_str()
            .unwrap(),
        "user"
    );
    assert_eq!(
        body.get("coachMessage")
            .unwrap()
            .get("role")
            .unwrap()
            .as_str()
            .unwrap(),
        "coach"
    );
}

#[tokio::test]
async fn send_message_rejects_when_availability_is_missing() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("workout-1")])
            .with_availability_configured(false),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/messages")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"Legs felt heavy today"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_message_rejects_when_real_settings_service_reports_missing_availability() {
    let settings_service = TestAvailabilitySettingsService::unconfigured();
    let service = WorkoutSummaryService::new(
        InMemoryWorkoutSummaryRepository::with_summary(existing_summary()),
        InMemoryCoachReplyOperationRepository::default(),
        TestClock,
        TestIdGenerator,
    )
    .with_settings_service(settings_service.clone());

    let app = workout_summary_test_app_with_settings(
        TestIdentityServiceWithSession::default(),
        service,
        Some(settings_service),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/messages")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"Legs felt heavy today"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
