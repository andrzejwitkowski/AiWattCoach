use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::{
        intervals_test_app_with_calendar_entries_and_completed_workouts,
        intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service,
        sample_workout_summary, InMemoryCalendarEntryViewRepository,
        InMemoryCompletedWorkoutRepository, TestWorkoutSummaryService,
    },
    fixtures::{get_json, sample_completed_workout, session_cookie},
    identity_fakes::TestIdentityServiceWithSession,
    intervals_fakes::TestIntervalsService,
};

#[tokio::test]
async fn list_completed_workouts_returns_canonical_workouts_for_authenticated_user() {
    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-11",
            None,
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    let activity = &body.as_array().unwrap()[0];
    assert_eq!(activity.get("id").unwrap().as_str(), Some("activity-11"));
    assert_eq!(activity.get("trainer").unwrap().as_bool(), Some(false));
    assert_eq!(
        activity.get("externalId").unwrap().as_str(),
        Some("external-activity-11")
    );
    assert_eq!(
        activity
            .get("metrics")
            .unwrap()
            .get("normalizedPowerWatts")
            .unwrap()
            .as_i64(),
        Some(238)
    );
}

#[tokio::test]
async fn list_completed_workouts_excludes_other_users_workouts() {
    let mut other_user_workout = sample_completed_workout("activity-99", None);
    other_user_workout.user_id = "user-2".to_string();

    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![
            sample_completed_workout("activity-11", None),
            other_user_workout,
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    let ids = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["activity-11"]);
}

#[tokio::test]
async fn list_completed_workouts_preserves_trainer_flag_from_canonical_workout() {
    let mut workout = sample_completed_workout("activity-31", None);
    workout.trainer = true;

    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![workout]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    let activity = &body.as_array().unwrap()[0];
    assert_eq!(activity.get("id").unwrap().as_str(), Some("activity-31"));
    assert_eq!(activity.get("trainer").unwrap().as_bool(), Some(true));
}

#[tokio::test]
async fn get_completed_workout_returns_canonical_workout_detail() {
    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-21",
            None,
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("name").unwrap().as_str(),
        Some("VO2 Session Completed")
    );
    assert_eq!(
        body.get("details")
            .unwrap()
            .get("streams")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn get_completed_workout_returns_404_for_other_users_workout() {
    let mut other_user_workout = sample_completed_workout("activity-98", None);
    other_user_workout.user_id = "user-2".to_string();

    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![other_user_workout]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-98")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_completed_workout_summary_returns_recap_for_authenticated_user() {
    let mut summary = sample_workout_summary("user-1", "activity-21");
    summary.workout_recap_text =
        Some("Strong aerobic control with a fade only near the end.".to_string());
    summary.workout_recap_provider = Some("openai".to_string());
    summary.workout_recap_model = Some("gpt-4.1".to_string());
    summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_200);

    let app = intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-21",
            None,
        )]),
        TestWorkoutSummaryService::with_summaries(vec![summary]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-21/summary")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("workoutId").and_then(Value::as_str),
        Some("activity-21")
    );
    assert_eq!(
        body.get("text").and_then(Value::as_str),
        Some("Strong aerobic control with a fade only near the end.")
    );
    assert_eq!(body.get("provider").and_then(Value::as_str), Some("openai"));
    assert_eq!(body.get("model").and_then(Value::as_str), Some("gpt-4.1"));
    assert_eq!(
        body.get("generatedAtEpochSeconds").and_then(Value::as_i64),
        Some(1_700_000_200)
    );
}

#[tokio::test]
async fn get_completed_workout_summary_returns_404_when_summary_is_missing() {
    let app = intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-22",
            None,
        )]),
        TestWorkoutSummaryService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-22/summary")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_completed_workout_summary_returns_404_when_recap_text_is_missing() {
    let summary = sample_workout_summary("user-1", "activity-23");

    let app = intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-23",
            None,
        )]),
        TestWorkoutSummaryService::with_summaries(vec![summary]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-23/summary")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_completed_workout_summary_returns_404_for_other_users_workout() {
    let mut other_user_workout = sample_completed_workout("activity-24", None);
    other_user_workout.user_id = "user-2".to_string();

    let mut other_user_summary = sample_workout_summary("user-2", "activity-24");
    other_user_summary.workout_recap_text = Some("Other user recap".to_string());
    other_user_summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_200);

    let app = intervals_test_app_with_calendar_entries_completed_workouts_and_summary_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![other_user_workout]),
        TestWorkoutSummaryService::with_summaries(vec![other_user_summary]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts/activity-24/summary")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_completed_workouts_rejects_reversed_date_ranges() {
    let app = intervals_test_app_with_calendar_entries_and_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::default(),
        InMemoryCompletedWorkoutRepository::with_workouts(vec![sample_completed_workout(
            "activity-11",
            None,
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/completed-workouts?oldest=2026-03-31&newest=2026-03-01")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
