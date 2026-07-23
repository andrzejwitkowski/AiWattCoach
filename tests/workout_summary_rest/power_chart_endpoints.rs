use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::shared::{
    session_cookie, workout_summary_test_app_with_completed_workouts,
    TestIdentityServiceWithSession, TestWorkoutSummaryService,
};
use aiwattcoach::domain::completed_workouts::{
    BoxFuture, CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutError,
    CompletedWorkoutMetrics, CompletedWorkoutReadUseCases, CompletedWorkoutSeries,
    CompletedWorkoutStream,
};

const RESPONSE_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Clone)]
struct StubCompletedWorkoutService {
    workout: Option<CompletedWorkout>,
}

impl CompletedWorkoutReadUseCases for StubCompletedWorkoutService {
    fn list_completed_workouts(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_completed_workout(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let workout = self.workout.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            Ok(workout.filter(|workout| {
                workout.user_id == user_id
                    && workout
                        .source_activity_id
                        .as_deref()
                        .map(|id| id == activity_id)
                        .unwrap_or(false)
            }))
        })
    }
}

fn watts_workout() -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: "wahoo-workout:1".to_string(),
        user_id: "user-1".to_string(),
        start_date_local: "2026-05-27T13:10:35.000Z".to_string(),
        source_activity_id: Some("i151959404".to_string()),
        planned_workout_id: None,
        name: Some("Aerobic Endurance".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        external_id: None,
        trainer: false,
        duration_seconds: Some(60),
        distance_meters: None,
        metrics: CompletedWorkoutMetrics {
            normalized_power_watts: Some(210),
            average_power_watts: Some(190),
            ..Default::default()
        },
        details: CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: None,
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                    100, 150, 200, 250, 300, 280, 260, 240,
                ])),
                secondary_series: None,
                value_type_is_array: true,
                custom: false,
                all_null: false,
            }],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
        power_curve_5s: None,
    }
}

fn no_watts_workout() -> CompletedWorkout {
    let mut workout = watts_workout();
    workout.details.streams = Vec::new();
    workout
}

async fn power_chart_response(service: StubCompletedWorkoutService) -> axum::response::Response {
    let app = workout_summary_test_app_with_completed_workouts(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::default(),
        Arc::new(service),
    )
    .await;

    app.oneshot(
        Request::builder()
            .uri("/api/workout-summaries/i151959404/power-chart.png")
            .header(header::COOKIE, session_cookie("session-1"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn power_chart_returns_png_when_watts_stream_present() {
    let response = power_chart_response(StubCompletedWorkoutService {
        workout: Some(watts_workout()),
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
    let body = to_bytes(response.into_body(), RESPONSE_LIMIT_BYTES)
        .await
        .unwrap();
    assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
}

#[tokio::test]
async fn power_chart_returns_not_found_without_watts_stream() {
    let response = power_chart_response(StubCompletedWorkoutService {
        workout: Some(no_watts_workout()),
    })
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn power_chart_returns_not_found_when_workout_missing() {
    let response = power_chart_response(StubCompletedWorkoutService { workout: None }).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
