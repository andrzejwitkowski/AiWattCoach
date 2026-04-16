use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    calendar::{CalendarError, HiddenCalendarEventSource},
    calendar_labels::{CalendarLabelError, CalendarLabelSource},
    intervals::DateRange,
    races::{
        BoxFuture as RaceBoxFuture, CreateRace, Race, RaceDiscipline, RaceError, RacePriority,
        RaceUseCases, UpdateRace,
    },
};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::util::ServiceExt;

use crate::{
    app::{intervals_test_app_with_all_services, EmptyTrainingPlanProjectionRepository},
    fixtures::{get_json, session_cookie},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::ScopedIntervalsService,
};

#[tokio::test]
async fn create_race_returns_created_race_payload() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service.clone(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/races")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"date":"2026-09-12","name":"Gravel Attack","distanceMeters":120000,"discipline":"gravel","priority":"B"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("name").and_then(|value| value.as_str()),
        Some("Gravel Attack")
    );
    assert!(body.get("linkedIntervalsEventId").is_none());
    assert!(body.get("syncStatus").is_none());
}

#[tokio::test]
async fn list_races_returns_all_races() {
    let race_service = RecordingRaceService::with_races(vec![sample_race()]);
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races?oldest=2026-09-01&newest=2026-09-30")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(
        body.as_array().unwrap()[0]
            .get("raceId")
            .and_then(|value| value.as_str()),
        Some("race-1")
    );
}

#[tokio::test]
async fn get_race_returns_race_for_existing_id() {
    let race_service = RecordingRaceService::with_races(vec![sample_race()]);
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races/race-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    assert_eq!(body.get("raceId").and_then(|v| v.as_str()), Some("race-1"));
}

#[tokio::test]
async fn get_race_returns_404_for_missing_id() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races/race-999")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_race_returns_updated_race_payload() {
    let race_service = RecordingRaceService::with_races(vec![sample_race()]);
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/races/race-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"date":"2026-09-20","name":"Updated Race","distanceMeters":90000,"discipline":"road","priority":"A"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("name").and_then(|v| v.as_str()),
        Some("Updated Race")
    );
}

#[tokio::test]
async fn delete_race_returns_204() {
    let race_service = RecordingRaceService::with_races(vec![sample_race()]);
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/races/race-1")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_race_returns_400_for_invalid_date() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/races")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"date":"not-a-date","name":"Bad Race","distanceMeters":10000,"discipline":"road","priority":"C"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_race_returns_400_for_blank_name() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/races")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"date":"2026-09-12","name":"   ","distanceMeters":10000,"discipline":"road","priority":"C"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_race_requires_authentication() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/races")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"date":"2026-09-12","name":"Gravel Attack","distanceMeters":120000,"discipline":"gravel","priority":"B"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_races_requires_authentication() {
    let race_service = RecordingRaceService::default();
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races?oldest=2026-09-01&newest=2026-09-30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_races_rejects_inverted_date_range() {
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        RecordingRaceService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races?oldest=2026-09-30&newest=2026-09-01")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_races_is_scoped_to_authenticated_user() {
    // The RecordingRaceService filters by user_id so user-2's session should
    // only receive races belonging to user-2.
    let race_service = RecordingRaceService::with_races(vec![
        sample_race(), // user-1
        Race {
            race_id: "race-user2".to_string(),
            user_id: "user-2".to_string(),
            ..sample_race()
        },
    ]);

    let app = intervals_test_app_with_all_services(
        SessionMappedIdentityService::with_users([
            ("session-user-1", "user-1", "user-1@example.com"),
            ("session-user-2", "user-2", "user-2@example.com"),
        ]),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        EmptyLabelSource,
        EmptyHiddenSource,
        race_service,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/races?oldest=2026-09-01&newest=2026-09-30")
                .header(header::COOKIE, session_cookie("session-user-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    let races = body.as_array().unwrap();
    assert_eq!(races.len(), 1);
    assert_eq!(
        races[0].get("raceId").and_then(|v| v.as_str()),
        Some("race-user2")
    );
}

#[derive(Clone, Default)]
struct EmptyLabelSource;

impl CalendarLabelSource for EmptyLabelSource {
    fn list_labels(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar_labels::BoxFuture<
        Result<Vec<aiwattcoach::domain::calendar_labels::CalendarLabel>, CalendarLabelError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
struct EmptyHiddenSource;

impl HiddenCalendarEventSource for EmptyHiddenSource {
    fn list_hidden_intervals_event_ids(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar::BoxFuture<Result<Vec<i64>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
struct RecordingRaceService {
    races: Arc<Mutex<Vec<Race>>>,
}

impl RecordingRaceService {
    fn with_races(races: Vec<Race>) -> Self {
        Self {
            races: Arc::new(Mutex::new(races)),
        }
    }
}

impl RaceUseCases for RecordingRaceService {
    fn list_races(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let user_id = user_id.to_string();
        let races = self.races.lock().unwrap().clone();
        Box::pin(async move {
            Ok(races
                .into_iter()
                .filter(|race| race.user_id == user_id)
                .collect())
        })
    }

    fn get_race(&self, _user_id: &str, race_id: &str) -> RaceBoxFuture<Result<Race, RaceError>> {
        let race_id = race_id.to_string();
        let races = self.races.lock().unwrap().clone();
        Box::pin(async move {
            races
                .into_iter()
                .find(|race| race.race_id == race_id)
                .ok_or(RaceError::NotFound)
        })
    }

    fn create_race(
        &self,
        user_id: &str,
        request: CreateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        let user_id = user_id.to_string();
        let races = self.races.clone();
        Box::pin(async move {
            let race = Race {
                race_id: "race-1".to_string(),
                user_id,
                date: request.date,
                name: request.name,
                distance_meters: request.distance_meters,
                discipline: request.discipline,
                priority: request.priority,
                result: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            };
            races.lock().unwrap().push(race.clone());
            Ok(race)
        })
    }

    fn update_race(
        &self,
        _user_id: &str,
        race_id: &str,
        request: UpdateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        let race_id = race_id.to_string();
        let races = self.races.clone();
        Box::pin(async move {
            let mut locked = races.lock().unwrap();
            let existing = locked
                .iter_mut()
                .find(|race| race.race_id == race_id)
                .ok_or(RaceError::NotFound)?;
            existing.name = request.name;
            existing.date = request.date;
            existing.distance_meters = request.distance_meters;
            existing.discipline = request.discipline;
            existing.priority = request.priority;
            Ok(existing.clone())
        })
    }

    fn delete_race(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        Box::pin(async { Ok(()) })
    }
}

fn sample_race() -> Race {
    Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}
