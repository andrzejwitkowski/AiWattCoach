use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    get_json, sample_summary, sample_summary_with_updated_at, session_cookie,
    workout_summary_test_app, TestIdentityServiceWithSession, TestWorkoutSummaryService,
};

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
                .uri("/api/workout-summaries/event-1")
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
                .uri("/api/workout-summaries/event-1")
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
                .uri("/api/workout-summaries/event-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = get_json(response).await;
    assert_eq!(body.get("eventId").unwrap().as_str().unwrap(), "event-1");
    assert!(body.get("messages").unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_summary_returns_existing_summary() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries/event-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert_eq!(body.get("id").unwrap().as_str().unwrap(), "summary-event-1");
    assert_eq!(body.get("rpe").unwrap().as_u64().unwrap(), 6);
}

#[tokio::test]
async fn list_summaries_returns_batch_results() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![
            sample_summary_with_updated_at("event-1", 1_700_000_050),
            sample_summary_with_updated_at("event-2", 1_700_000_100),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workout-summaries?eventIds=event-1,event-2,event-3")
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
        summaries[0].get("eventId").unwrap().as_str().unwrap(),
        "event-2"
    );
    assert_eq!(
        summaries[1].get("eventId").unwrap().as_str().unwrap(),
        "event-1"
    );
}

#[tokio::test]
async fn update_rpe_returns_updated_summary() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/workout-summaries/event-1/rpe")
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
async fn send_message_returns_persisted_turn() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/event-1/messages")
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
