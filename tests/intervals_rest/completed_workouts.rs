use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::{
        intervals_test_app_with_calendar_entries_and_completed_workouts,
        InMemoryCalendarEntryViewRepository, InMemoryCompletedWorkoutRepository,
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
async fn get_completed_workout_uses_legacy_completed_workout_id_fallback() {
    let mut workout = sample_completed_workout("intervals-activity:legacy-41", None);
    workout.source_activity_id = None;

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
                .uri("/api/completed-workouts/legacy-41")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("id").unwrap().as_str(), Some("legacy-41"));
}

#[tokio::test]
async fn get_completed_workout_accepts_canonical_completed_workout_ids() {
    let mut workout = sample_completed_workout("intervals-activity:legacy-42", None);
    workout.source_activity_id = None;

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
                .uri("/api/completed-workouts/intervals-activity:legacy-42")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("id").unwrap().as_str(), Some("legacy-42"));
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
