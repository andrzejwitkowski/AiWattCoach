use aiwattcoach::domain::{
    calendar_view::CalendarEntryKind,
    intervals::{parse_planned_workout, IntervalsError},
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
        InMemoryCompletedWorkoutRepository,
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
async fn refresh_calendar_view_requires_authentication() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/refresh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_calendar_view_returns_refresh_summary_for_authenticated_user() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/refresh")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("oldest").and_then(|value| value.as_str()),
        Some("2026-01-01")
    );
    assert_eq!(
        body.get("newest").and_then(|value| value.as_str()),
        Some("2026-04-27")
    );
    assert_eq!(
        body.get("rebuiltEntryCount")
            .and_then(|value| value.as_u64()),
        Some(3)
    );
}

#[tokio::test]
async fn refresh_calendar_view_returns_error_message_body_when_refresh_fails() {
    #[derive(Clone)]
    struct FailingManualCalendarRefreshService;

    impl aiwattcoach::domain::calendar_view::ManualCalendarRefreshUseCases
        for FailingManualCalendarRefreshService
    {
        fn refresh_calendar_view_for_user(
            &self,
            _user_id: &str,
        ) -> aiwattcoach::domain::calendar_view::BoxFuture<
            Result<
                aiwattcoach::domain::calendar_view::ManualCalendarRefreshResult,
                aiwattcoach::domain::calendar_view::CalendarEntryViewError,
            >,
        > {
            Box::pin(async {
                Err(
                    aiwattcoach::domain::calendar_view::CalendarEntryViewError::Repository(
                        "calendar refresh failed in test".to_string(),
                    ),
                )
            })
        }
    }

    let app = crate::app::intervals_test_app_with_manual_calendar_refresh_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        std::sync::Arc::new(FailingManualCalendarRefreshService),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/refresh")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("message").and_then(|value| value.as_str()),
        Some("failed to refresh calendar view")
    );
    assert!(body.get("code").is_none());
}

#[tokio::test]
async fn refresh_calendar_view_is_scoped_to_authenticated_user() {
    #[derive(Clone)]
    struct RecordingManualCalendarRefreshService {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl aiwattcoach::domain::calendar_view::ManualCalendarRefreshUseCases
        for RecordingManualCalendarRefreshService
    {
        fn refresh_calendar_view_for_user(
            &self,
            user_id: &str,
        ) -> aiwattcoach::domain::calendar_view::BoxFuture<
            Result<
                aiwattcoach::domain::calendar_view::ManualCalendarRefreshResult,
                aiwattcoach::domain::calendar_view::CalendarEntryViewError,
            >,
        > {
            let calls = self.calls.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                calls.lock().unwrap().push(user_id.clone());
                Ok(
                    aiwattcoach::domain::calendar_view::ManualCalendarRefreshResult {
                        oldest: "2026-01-01".to_string(),
                        newest: "2026-04-27".to_string(),
                        rebuilt_entry_count: 1,
                    },
                )
            })
        }
    }

    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let service = RecordingManualCalendarRefreshService {
        calls: calls.clone(),
    };

    let app = crate::app::intervals_test_app_with_manual_calendar_refresh_service(
        SessionMappedIdentityService::with_users([
            ("session-user-1", "user-1", "user-1@example.com"),
            ("session-user-2", "user-2", "user-2@example.com"),
        ]),
        TestIntervalsService::default(),
        Arc::new(service),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/refresh")
                .header(header::COOKIE, session_cookie("session-user-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0], "user-2");
}

#[tokio::test]
async fn refresh_calendar_view_returns_generic_error_for_invariant_violation() {
    #[derive(Clone)]
    struct InvariantViolationRefreshService;

    impl aiwattcoach::domain::calendar_view::ManualCalendarRefreshUseCases
        for InvariantViolationRefreshService
    {
        fn refresh_calendar_view_for_user(
            &self,
            _user_id: &str,
        ) -> aiwattcoach::domain::calendar_view::BoxFuture<
            Result<
                aiwattcoach::domain::calendar_view::ManualCalendarRefreshResult,
                aiwattcoach::domain::calendar_view::CalendarEntryViewError,
            >,
        > {
            Box::pin(async {
                Err(
                    aiwattcoach::domain::calendar_view::CalendarEntryViewError::InvariantViolation(
                        "race data unauthenticated".to_string(),
                    ),
                )
            })
        }
    }

    let app = crate::app::intervals_test_app_with_manual_calendar_refresh_service(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        std::sync::Arc::new(InvariantViolationRefreshService),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/refresh")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("message").and_then(|value| value.as_str()),
        Some("failed to refresh calendar view")
    );
    assert!(body.get("code").is_none());
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
    assert!(event.get("actualWorkout").unwrap().is_null());
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
async fn list_calendar_events_expands_canonical_repeat_blocks_in_event_definition_summary() {
    let app = intervals_test_app_with_calendar_entries(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
        InMemoryCalendarEntryViewRepository::with_entries(vec![sample_planned_calendar_entry(
            "planned:intervals-event:14",
            "2026-03-22",
            "Repeatability Under Fatigue",
            "Main Set 2x\n- 10m 95%\n- 3m 55%\nCooldown\n- 5m 50%",
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
    let event_definition = event.get("eventDefinition").unwrap();
    let summary = event_definition.get("summary").unwrap();
    let segments = event_definition
        .get("segments")
        .unwrap()
        .as_array()
        .unwrap();

    assert_eq!(
        summary.get("totalDurationSeconds").unwrap().as_i64(),
        Some(1_860)
    );
    assert_eq!(summary.get("totalSegments").unwrap().as_u64(), Some(5));
    assert_eq!(
        segments[0].get("label").unwrap().as_str(),
        Some("10m 95% #1")
    );
    assert_eq!(
        segments[1].get("label").unwrap().as_str(),
        Some("3m 55% #1")
    );
    assert_eq!(
        segments[2].get("label").unwrap().as_str(),
        Some("10m 95% #2")
    );
    assert_eq!(
        segments[3].get("label").unwrap().as_str(),
        Some("3m 55% #2")
    );
    assert_eq!(segments[4].get("label").unwrap().as_str(), Some("5m 50%"));
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
        InMemoryCompletedWorkoutRepository::default(),
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
async fn sync_planned_workout_requires_cycling_ftp_settings() {
    let intervals_service = ScopedIntervalsService::default();
    let app = intervals_test_app_with_projections(
        TestIdentityServiceWithSession::default(),
        intervals_service.clone(),
        TestTrainingPlanProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-16",
            "Build Session",
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/planned-workouts/training-plan:user-1:w1:1/2023-11-16/wahoo/sync")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("message").and_then(|value| value.as_str()),
        Some("Set your cycling FTP in Settings before syncing to Wahoo")
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
            "2023-11-16",
            "User 1 Workout",
        )]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/planned-workouts/shared-operation/2023-11-16/intervals/sync")
                .header(header::COOKIE, session_cookie("session-user-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sync_planned_workout_returns_validation_message_for_invalid_date() {
    let app = intervals_test_app_with_projections(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        TestTrainingPlanProjectionRepository::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/calendar/planned-workouts/training-plan:user-1:w1:1/not-a-date/intervals/sync")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("code").and_then(|value| value.as_str()),
        Some("invalid_date_format")
    );
    assert_eq!(
        body.get("message").and_then(|value| value.as_str()),
        Some("planned workout date must be in YYYY-MM-DD format")
    );
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
            aiwattcoach::domain::training_plan::TrainingPlanReplacementResult,
            TrainingPlanError,
        >,
    > {
        Box::pin(async move {
            Ok(
                aiwattcoach::domain::training_plan::TrainingPlanReplacementResult {
                    snapshot,
                    projected_days,
                    superseded_date_range: None,
                },
            )
        })
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
        rest_day_reason: None,
        workout: Some(build_planned_workout(workout_name)),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

fn build_planned_workout(name: &str) -> aiwattcoach::domain::intervals::PlannedWorkout {
    parse_planned_workout(&format!("{name}\n- 60m 70%")).expect("planned workout should parse")
}
