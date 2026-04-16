use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    calendar::{CalendarError, HiddenCalendarEventSource},
    calendar_labels::{
        CalendarLabel, CalendarLabelError, CalendarLabelPayload, CalendarLabelSource,
        CalendarRaceLabel,
    },
    intervals::{DateRange, Event, EventCategory},
    races::{BoxFuture as RaceBoxFuture, CreateRace, Race, RaceError, RaceUseCases, UpdateRace},
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
async fn list_calendar_labels_returns_race_labels_grouped_by_date() {
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::with_labels(vec![race_label("race-1", "2026-03-22", "Tatra Road Race")]),
        EmptyHiddenSource,
        StubRaceService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/labels?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = get_json(response).await;
    assert_eq!(
        body.get("labelsByDate")
            .and_then(|value| value.get("2026-03-22"))
            .and_then(|value| value.get("race:race-1"))
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str()),
        Some("race")
    );
    assert_eq!(
        body.get("labelsByDate")
            .and_then(|value| value.get("2026-03-22"))
            .and_then(|value| value.get("race:race-1"))
            .and_then(|value| value.get("title"))
            .and_then(|value| value.as_str()),
        Some("Race Tatra Road Race")
    );
    assert_eq!(
        body.get("labelsByDate")
            .and_then(|value| value.get("2026-03-22"))
            .and_then(|value| value.get("race:race-1"))
            .and_then(|value| value.get("payload"))
            .and_then(|value| value.get("syncStatus"))
            .and_then(|value| value.as_str()),
        Some("pending")
    );
    assert!(body
        .get("labelsByDate")
        .and_then(|value| value.get("2026-03-22"))
        .and_then(|value| value.get("race:race-1"))
        .and_then(|value| value.get("payload"))
        .and_then(|value| value.get("linkedIntervalsEventId"))
        .is_some_and(|value| value.is_null()));
}

#[tokio::test]
async fn list_calendar_events_hides_intervals_events_linked_to_labels() {
    let hidden_source = HiddenIdsSource::with_hidden_ids(vec![41]);
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::with_user_events([(
            "user-1",
            vec![Event {
                id: 41,
                start_date_local: "2026-03-22".to_string(),
                event_type: Some("Ride".to_string()),
                name: Some("Race Tatra Road Race".to_string()),
                category: EventCategory::Race,
                description: None,
                indoor: false,
                color: None,
                workout_doc: None,
            }],
        )]),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::default(),
        hidden_source,
        StubRaceService,
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
async fn list_calendar_labels_returns_400_for_invalid_date() {
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::default(),
        EmptyHiddenSource,
        StubRaceService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/labels?oldest=not-a-date&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_calendar_labels_rejects_inverted_date_range() {
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::default(),
        EmptyHiddenSource,
        StubRaceService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/labels?oldest=2026-03-31&newest=2026-03-01")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_calendar_labels_requires_authentication() {
    let app = intervals_test_app_with_all_services(
        TestIdentityServiceWithSession::default(),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::default(),
        EmptyHiddenSource,
        StubRaceService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/labels?oldest=2026-03-01&newest=2026-03-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_calendar_labels_is_scoped_to_authenticated_user() {
    // The RaceLabelSource scopes results by user_id (labels belong to user-1).
    // user-2's session must receive an empty labelsByDate map.
    let app = intervals_test_app_with_all_services(
        SessionMappedIdentityService::with_users([
            ("session-user-1", "user-1", "user-1@example.com"),
            ("session-user-2", "user-2", "user-2@example.com"),
        ]),
        ScopedIntervalsService::default(),
        EmptyTrainingPlanProjectionRepository,
        RaceLabelSource::with_labels(vec![race_label("race-1", "2026-03-22", "Tatra Road Race")]),
        EmptyHiddenSource,
        StubRaceService,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/calendar/labels?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-user-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = get_json(response).await;
    let labels_by_date = body.get("labelsByDate").unwrap();
    assert!(
        labels_by_date
            .as_object()
            .map(|m| m.is_empty())
            .unwrap_or(false),
        "user-2 should receive no labels"
    );
}

#[derive(Clone, Default)]
struct RaceLabelSource {
    labels: Arc<Mutex<Vec<(String, CalendarLabel)>>>,
}

impl RaceLabelSource {
    fn with_labels(labels: Vec<CalendarLabel>) -> Self {
        // Labels owned by "user-1" by default
        Self {
            labels: Arc::new(Mutex::new(
                labels
                    .into_iter()
                    .map(|l| ("user-1".to_string(), l))
                    .collect(),
            )),
        }
    }
}

impl CalendarLabelSource for RaceLabelSource {
    fn list_labels(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar_labels::BoxFuture<
        Result<Vec<CalendarLabel>, CalendarLabelError>,
    > {
        let user_id = user_id.to_string();
        let labels = self.labels.lock().unwrap().clone();
        Box::pin(async move {
            Ok(labels
                .into_iter()
                .filter(|(owner, _)| *owner == user_id)
                .map(|(_, label)| label)
                .collect())
        })
    }
}

#[derive(Clone, Default)]
struct HiddenIdsSource {
    ids: Arc<Mutex<Vec<i64>>>,
}

impl HiddenIdsSource {
    fn with_hidden_ids(ids: Vec<i64>) -> Self {
        Self {
            ids: Arc::new(Mutex::new(ids)),
        }
    }
}

impl HiddenCalendarEventSource for HiddenIdsSource {
    fn list_hidden_intervals_event_ids(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar::BoxFuture<Result<Vec<i64>, CalendarError>> {
        let ids = self.ids.lock().unwrap().clone();
        Box::pin(async move { Ok(ids) })
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
struct StubRaceService;

impl RaceUseCases for StubRaceService {
    fn list_races(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_race(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::NotFound) })
    }

    fn create_race(
        &self,
        _user_id: &str,
        _request: CreateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::Unavailable("not used".to_string())) })
    }

    fn update_race(
        &self,
        _user_id: &str,
        _race_id: &str,
        _request: UpdateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::Unavailable("not used".to_string())) })
    }

    fn delete_race(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        Box::pin(async { Ok(()) })
    }
}

fn race_label(race_id: &str, date: &str, name: &str) -> CalendarLabel {
    CalendarLabel {
        label_key: format!("race:{race_id}"),
        date: date.to_string(),
        title: format!("Race {name}"),
        subtitle: Some("120 km • Kat. A".to_string()),
        payload: CalendarLabelPayload::Race(CalendarRaceLabel {
            race_id: race_id.to_string(),
            date: date.to_string(),
            name: name.to_string(),
            distance_meters: 120_000,
            discipline: "road".to_string(),
            priority: "A".to_string(),
            sync_status: "pending".to_string(),
            linked_intervals_event_id: None,
        }),
    }
}
