use aiwattcoach::domain::{
    calendar_view::CalendarEntryKind,
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
    app::{
        intervals_test_app, intervals_test_app_with_calendar_entries,
        intervals_test_app_with_projections,
        intervals_test_app_with_projections_and_calendar_entries, sample_calendar_entry,
        sample_planned_calendar_entry, InMemoryCalendarEntryViewRepository,
    },
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
async fn list_calendar_events_returns_local_planned_entries_for_authenticated_user() {
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::with_entries(vec![sample_planned_calendar_entry(
            "planned:intervals-event:11",
            "2026-03-22",
            "VO2 Session",
            "- 10min 55%",
        )]),
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
    assert_eq!(event.get("syncStatus").unwrap().as_str(), Some("synced"));
    assert_eq!(
        event.get("startDateLocal").unwrap().as_str(),
        Some("2026-03-22")
    );
}

#[tokio::test]
async fn list_calendar_events_parse_event_definition_from_description_when_workout_doc_is_blank() {
    let mut entry = sample_planned_calendar_entry(
        "planned:intervals-event:12",
        "2026-03-22",
        "Fallback Workout",
        "  \n\t ",
    );
    entry.description = Some("- 12min 60%".to_string());
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::with_entries(vec![entry]),
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
        event
            .get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .get("definition")
            .unwrap()
            .as_str(),
        Some("- 12min 60%")
    );
    assert_eq!(
        event
            .get("eventDefinition")
            .unwrap()
            .get("rawWorkoutDoc")
            .unwrap()
            .as_str(),
        Some("  \n\t ")
    );
}

#[tokio::test]
async fn list_calendar_events_does_not_return_completed_calendar_entries_as_standalone_events() {
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::with_entries(vec![sample_calendar_entry(
            "completed:completed-1",
            CalendarEntryKind::CompletedWorkout,
            "2026-03-22",
        )]),
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
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_calendar_events_normalizes_priority_race_categories_for_rest_clients() {
    let mut race_entry =
        sample_calendar_entry("race:race-11", CalendarEntryKind::Race, "2026-03-22");
    race_entry.title = "Priority Race".to_string();
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::with_entries(vec![race_entry]),
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
    assert_eq!(event.get("category").unwrap().as_str(), Some("RACE"));
}

#[tokio::test]
async fn create_event_rejects_priority_race_categories_for_rest_clients() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let request_body = serde_json::json!({
        "category": "RACE_B",
        "startDateLocal": "2026-03-25",
        "name": "Priority Race",
        "indoor": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_calendar_events_uses_local_entries_without_intervals_credentials() {
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_error(IntervalsError::CredentialsNotConfigured),
        InMemoryCalendarEntryViewRepository::with_entries(vec![sample_planned_calendar_entry(
            "planned:intervals-event:13",
            "2026-03-22",
            "Credentialless Workout",
            "- 10min 55%",
        )]),
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
        event.get("name").unwrap().as_str(),
        Some("Credentialless Workout")
    );
}

#[tokio::test]
async fn list_calendar_events_returns_predicted_events_with_positive_safe_ids() {
    let app = intervals_test_app_with_projections_and_calendar_entries(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        TestTrainingPlanProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1:1775719860",
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryCalendarEntryViewRepository::with_entries(vec![sample_planned_calendar_entry(
            "planned:training-plan:user-1:w1:1:1775719860:2026-03-26",
            "2026-03-26",
            "Build Session",
            "Build Session\n- 60m 70%",
        )]),
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
    let event_id = event.get("id").and_then(|value| value.as_i64()).unwrap();

    assert!(event_id > 0);
    assert!(event_id <= 9_007_199_254_740_991);
    assert_eq!(
        event.get("plannedSource").unwrap().as_str(),
        Some("predicted")
    );
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
    assert_eq!(created_event[0].start_date_local, "2026-03-26T00:00:00");
    assert_eq!(created_event[0].name.as_deref(), Some("Build Session"));
    assert_eq!(created_event[0].workout_doc, None);
    assert_eq!(created_event[0].description.as_deref(), Some("- 60m 70%"));
}

#[tokio::test]
async fn sync_planned_workout_preserves_existing_remote_description() {
    let intervals_service = ScopedIntervalsService::with_user_events([(
        "user-1",
        vec![Event {
            id: 41,
            start_date_local: "2026-03-26".to_string(),
            event_type: Some("Ride".to_string()),
            name: Some("Build Session".to_string()),
            category: EventCategory::Workout,
            description: Some("Keep this description".to_string()),
            indoor: true,
            color: Some("blue".to_string()),
            workout_doc: Some(serialize_planned_workout(&build_planned_workout(
                "Build Session",
            ))),
        }],
    )]);
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

    let updated_events = intervals_service
        .list_events(
            "user-1",
            &aiwattcoach::domain::intervals::DateRange {
                oldest: "2026-03-26".to_string(),
                newest: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated_events.len(), 2);
    let updated_event = updated_events
        .iter()
        .find(|event| event.id == 41)
        .expect("existing event should be updated in place");
    assert_eq!(updated_event.start_date_local, "2026-03-26");
    assert_eq!(updated_event.name.as_deref(), Some("Build Session"));
    assert_eq!(
        updated_event.description.as_deref(),
        Some("Keep this description")
    );
    assert!(updated_event.indoor);
    assert_eq!(updated_event.color.as_deref(), Some("blue"));
    assert_eq!(
        updated_event.workout_doc.as_deref(),
        Some(serialize_planned_workout(&build_planned_workout("Build Session")).as_str())
    );

    let synced_event = updated_events
        .iter()
        .find(|event| event.id != 41)
        .expect("synced event copy should be created");
    assert_eq!(synced_event.start_date_local, "2026-03-26T00:00:00");
    assert_eq!(synced_event.description.as_deref(), Some("- 60m 70%"));
    assert_eq!(synced_event.workout_doc, None);
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
