use aiwattcoach::domain::intervals::ActivityInterval;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::intervals_test_app,
    fixtures::{get_json, sample_activity, sample_event, session_cookie, watts_stream},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::{ScopedIntervalsService, TestIntervalsService},
};

#[tokio::test]
async fn get_event_returns_single_event() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(
            21,
            "Threshold",
            Some("- 20min 95%".to_string()),
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str().unwrap(), "Threshold");
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .get("definition")
            .unwrap()
            .as_str()
            .unwrap(),
        "- 20min 95%"
    );
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("segments")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("summary")
            .unwrap()
            .get("totalDurationSeconds")
            .unwrap()
            .as_i64(),
        Some(1200)
    );
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("summary")
            .unwrap()
            .get("estimatedIntensityFactor")
            .unwrap()
            .as_f64(),
        Some(0.95)
    );
    assert!(body.get("actualWorkout").unwrap().is_null());
}

#[tokio::test]
async fn get_event_includes_actual_workout_when_matching_activity_exists() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events_and_activities(
            vec![sample_event(
                21,
                "Threshold",
                Some("- 20min 95%".to_string()),
            )],
            vec![sample_activity("i21", "Threshold Ride")],
        ),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("actualWorkout")
            .unwrap()
            .get("activityId")
            .unwrap()
            .as_str(),
        Some("i21")
    );
    assert_eq!(
        body.get("actualWorkout")
            .unwrap()
            .get("matchedIntervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn get_event_hydrates_actual_workout_from_detailed_activity_lookup() {
    let mut listed_activity = sample_activity("i21", "Threshold Ride");
    listed_activity.details.streams = Vec::new();
    let mut detailed_activity = sample_activity("i21", "Threshold Ride");
    detailed_activity.details.streams = vec![watts_stream(&[120, 240, 280, 250])];

    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events_listed_and_detailed_activities(
            vec![sample_event(
                21,
                "Threshold",
                Some("- 20min 95%".to_string()),
            )],
            vec![listed_activity],
            vec![detailed_activity],
        ),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("actualWorkout")
            .unwrap()
            .get("powerValues")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[tokio::test]
async fn get_event_hydrates_actual_workout_from_detailed_activity_lookup_without_list_match() {
    let mut listed_activity = sample_activity("i21", "Threshold Ride");
    listed_activity.details.intervals = Vec::new();
    listed_activity.details.streams = Vec::new();

    let mut detailed_activity = sample_activity("i21", "Threshold Ride");
    detailed_activity.details.intervals = vec![ActivityInterval {
        id: Some(7),
        label: Some("Threshold".to_string()),
        interval_type: Some("WORK".to_string()),
        group_id: Some("g2".to_string()),
        start_index: Some(100),
        end_index: Some(1300),
        start_time_seconds: Some(600),
        end_time_seconds: Some(1800),
        moving_time_seconds: Some(1200),
        elapsed_time_seconds: Some(1200),
        distance_meters: Some(12000.0),
        average_power_watts: Some(271),
        normalized_power_watts: Some(280),
        training_stress_score: Some(35.0),
        average_heart_rate_bpm: Some(161),
        average_cadence_rpm: Some(89.0),
        average_speed_mps: Some(10.1),
        average_stride_meters: None,
        zone: Some(4),
    }];
    detailed_activity.details.streams = vec![watts_stream(&[120, 240, 280, 250])];

    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events_listed_and_detailed_activities(
            vec![sample_event(
                21,
                "Threshold",
                Some("- 20min 95%".to_string()),
            )],
            vec![listed_activity],
            vec![detailed_activity],
        ),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("actualWorkout")
            .unwrap()
            .get("activityId")
            .unwrap()
            .as_str(),
        Some("i21")
    );
}

#[tokio::test]
async fn get_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    501,
                    "User One Workout",
                    Some("- 1x20min 90%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    502,
                    "User Two Workout",
                    Some("- 6x2min 130%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/502")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_event_returns_404_when_not_found() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events/999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
