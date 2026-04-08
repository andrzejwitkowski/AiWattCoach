use aiwattcoach::domain::{
    intervals::{
        parse_planned_workout, serialize_planned_workout, Event, EventCategory, IntervalsError,
        IntervalsUseCases,
    },
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
    },
};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

use crate::{
    app::{intervals_test_app, intervals_test_app_with_projections},
    fixtures::{get_json, session_cookie},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::{ScopedIntervalsService, TestIntervalsService},
};

#[tokio::test]
async fn list_calendar_events_requires_authentication() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/events?oldest=2026-03-01&newest=2026-03-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_calendar_events_returns_intervals_events_for_authenticated_user() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![Event {
            id: 11,
            start_date_local: "2026-03-22".to_string(),
            name: Some("VO2 Session".to_string()),
            category: EventCategory::Workout,
            description: None,
            indoor: true,
            color: None,
            workout_doc: Some("- 10min 55%".to_string()),
        }]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = get_json(response).await;
    let event = &body.as_array().unwrap()[0];
    assert_eq!(
        event.get("plannedSource").unwrap().as_str(),
        Some("intervals")
    );
    assert!(event.get("syncStatus").unwrap().is_null());
}

#[tokio::test]
async fn list_calendar_events_reports_missing_credentials_as_unprocessable_entity() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::CredentialsNotConfigured),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn sync_planned_workout_returns_synced_calendar_event() {
    let intervals_service = ScopedIntervalsService::default();
    let app = intervals_test_app_with_projections(
        TestIdentityServiceWithSession::default(),
        intervals_service.clone(),
        TestTrainingPlanProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2026-03-26",
            "Build Session",
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/planned-workouts/training-plan:user-1:w1:1/2026-03-26/sync")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let event: serde_json::Value = get_json(response).await;
    assert_eq!(
        event.get("plannedSource").unwrap().as_str(),
        Some("predicted")
    );
    assert_eq!(event.get("syncStatus").unwrap().as_str(), Some("synced"));
    assert_eq!(
        event.get("linkedIntervalsEventId").unwrap().as_i64(),
        Some(1)
    );
    assert_eq!(
        event
            .get("projectedWorkout")
            .and_then(|value| value.get("operationKey"))
            .and_then(|value| value.as_str()),
        Some("training-plan:user-1:w1:1")
    );

    let created_event = intervals_service
        .list_events(
            "user-1",
            &aiwattcoach::domain::intervals::DateRange {
                oldest: "2026-03-26".to_string(),
                newest: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(created_event.len(), 1);
    assert_eq!(created_event[0].name.as_deref(), Some("Build Session"));
    assert_eq!(
        created_event[0].workout_doc.as_deref(),
        Some(serialize_planned_workout(&build_planned_workout("Build Session")).as_str())
    );
}

#[tokio::test]
async fn sync_planned_workout_is_scoped_to_authenticated_user() {
    let app = intervals_test_app_with_projections(
        SessionMappedIdentityService::with_users([
            ("session-user-1", "user-1", "user-1@example.com"),
            ("session-user-2", "user-2", "user-2@example.com"),
        ]),
        ScopedIntervalsService::default(),
        TestTrainingPlanProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "shared-operation",
            "2026-03-26",
            "User 1 Workout",
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/planned-workouts/shared-operation/2026-03-26/sync")
                .header(header::COOKIE, session_cookie("session-user-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[derive(Clone, Default)]
struct TestTrainingPlanProjectionRepository {
    days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
}

impl TestTrainingPlanProjectionRepository {
    fn with_days(days: Vec<TrainingPlanProjectedDay>) -> Self {
        Self {
            days: Arc::new(Mutex::new(days)),
        }
    }
}

impl TrainingPlanProjectionRepository for TestTrainingPlanProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let user_id = user_id.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id && day.superseded_at_epoch_seconds.is_none())
                .collect())
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let operation_key = operation_key.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| {
                    day.operation_key == operation_key && day.superseded_at_epoch_seconds.is_none()
                })
                .collect())
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| {
                    day.user_id == user_id
                        && day.operation_key == operation_key
                        && day.superseded_at_epoch_seconds.is_none()
                })
                .collect())
        })
    }

    fn replace_window(
        &self,
        snapshot: aiwattcoach::domain::training_plan::TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<
            (
                aiwattcoach::domain::training_plan::TrainingPlanSnapshot,
                Vec<TrainingPlanProjectedDay>,
            ),
            TrainingPlanError,
        >,
    > {
        Box::pin(async move { Ok((snapshot, projected_days)) })
    }
}

fn projected_day(
    user_id: &str,
    operation_key: &str,
    date: &str,
    workout_name: &str,
) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: user_id.to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: operation_key.to_string(),
        date: date.to_string(),
        rest_day: false,
        workout: Some(build_planned_workout(workout_name)),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

fn build_planned_workout(name: &str) -> aiwattcoach::domain::intervals::PlannedWorkout {
    parse_planned_workout(&format!("{name}\n- 60m 70%")).expect("planned workout should parse")
}
