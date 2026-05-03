use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar::{
        CalendarError, CalendarService, CalendarUseCases, NoopWahooUseCases,
        PlannedWorkoutSyncProvider, SyncPlannedWorkout,
    },
    calendar_view::{
        CalendarEntryKind, CalendarEntrySync, CalendarEntryView, CalendarEntryViewError,
        CalendarEntryViewRefreshPort, CalendarEntryViewRepository,
    },
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutError, CompletedWorkoutMetrics,
        CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
    },
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository,
    },
    identity::Clock,
    intervals::{
        parse_planned_workout, BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event,
        EventCategory, IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    planned_workout_tokens::{NoopPlannedWorkoutTokenRepository, PlannedWorkoutToken},
    settings::{CyclingSettings, SettingsError, UserSettings, UserSettingsRepository, WahooConfig},
    training_plan::{
        BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
        TrainingPlanProjectionRepository, TrainingPlanReplacementResult, TrainingPlanSnapshot,
    },
    wahoo::{
        WahooCreatePlan, WahooCreateWorkout, WahooError, WahooPlan, WahooUpdatePlan,
        WahooUpdateWorkout, WahooUseCases, WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
    },
};

#[tokio::test]
async fn sync_planned_workout_refreshes_calendar_view_for_synced_day() {
    let refresh = RecordingCalendarRefresh::default();
    let wahoo = RecordingWahooService::successful();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let settings = InMemoryUserSettingsRepository::with_ftp(295);
    let service = CalendarService::new(
        FakeIntervalsService::with_created_event(Event {
            id: 77,
            start_date_local: "2023-11-14T00:00:00".to_string(),
            event_type: Some("Ride".to_string()),
            name: Some("Build Session".to_string()),
            category: EventCategory::Workout,
            description: Some("- 60m 70%".to_string()),
            indoor: false,
            color: None,
            workout_doc: None,
        }),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states.clone(),
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), settings)
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default())
    .with_calendar_view_refresh(refresh.clone());

    let result = service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.linked_intervals_event_id, None);
    let wahoo_sync = sync_states
        .find_by_provider_and_canonical_entity(
            "user-1",
            ExternalProvider::Wahoo,
            &planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
        )
        .await
        .unwrap()
        .expect("expected wahoo sync record");
    assert_eq!(wahoo_sync.wahoo_plan_id, Some(5001));
    assert_eq!(wahoo_sync.wahoo_workout_id, Some(6001));
    assert_eq!(wahoo.plan_create_calls(), 1);
    assert_eq!(wahoo.workout_create_calls(), 1);
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-14".to_string(),
            "2023-11-14".to_string(),
        )]
    );
}

#[tokio::test]
async fn sync_planned_workout_refreshes_calendar_view_for_failed_day_after_persisting_failure() {
    let refresh = RecordingCalendarRefresh::default();
    let wahoo = RecordingWahooService::failing("wahoo unavailable");
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let settings = InMemoryUserSettingsRepository::with_ftp(295);
    let service = CalendarService::new(
        FakeIntervalsService::with_events_error(IntervalsError::ConnectionError(
            "intervals unused in Wahoo sync failure path".to_string(),
        )),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states.clone(),
        FixedClock,
    )
    .with_wahoo(wahoo, settings)
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default())
    .with_calendar_view_refresh(refresh.clone());

    let error = service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        CalendarError::Unavailable("wahoo unavailable".to_string())
    );
    let wahoo_sync = sync_states
        .find_by_provider_and_canonical_entity(
            "user-1",
            ExternalProvider::Wahoo,
            &planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
        )
        .await
        .unwrap()
        .expect("expected failed wahoo sync record");
    assert_eq!(wahoo_sync.wahoo_plan_id, None);
    assert_eq!(wahoo_sync.wahoo_workout_id, None);
    assert_eq!(wahoo_sync.last_error.as_deref(), Some("wahoo unavailable"));
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-14".to_string(),
            "2023-11-14".to_string(),
        )]
    );
}

#[tokio::test]
async fn sync_planned_workout_to_intervals_sends_structured_workout_as_workout_doc() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("- 60m 70%".to_string()),
        indoor: false,
        color: None,
        workout_doc: None,
    });
    let service = CalendarService::new(
        intervals.clone(),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        InMemoryExternalSyncStateRepository::default(),
        FixedClock,
    )
    .with_calendar_view_refresh(RecordingCalendarRefresh::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Intervals,
            },
        )
        .await
        .unwrap();

    let created = intervals.created_events.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].start_date_local, "2023-11-14T00:00:00");
    assert_eq!(created[0].description, None);
    assert_eq!(
        created[0].workout_doc.as_deref(),
        Some("Build Session\n- 60m 70%")
    );
}

#[tokio::test]
async fn sync_planned_workout_to_intervals_updates_existing_event_workout_doc() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2026-05-05T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Stochastic Durability - Over/Unders".to_string()),
        category: EventCategory::Workout,
        description: Some("manual note".to_string()),
        indoor: false,
        color: None,
        workout_doc: Some("old workout".to_string()),
    });
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Intervals,
                planned_workout_entity("training-plan:user-1:w1:1", "2026-05-05"),
            )
            .mark_synced("77".to_string(), "old-hash".to_string(), 1_700_000_001),
        )
        .await
        .unwrap();
    let service = CalendarService::new(
        intervals.clone(),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day_with_doc(
            "user-1",
            "training-plan:user-1:w1:1",
            "2026-05-05",
            "Stochastic Durability - Over/Unders\nWarmup\n- 15m ramp 175-250W\nMain Set 4x\n- 2m 105%\n- 4m 92%\n- 4m 50%\nCooldown\n- 15m 55%",
        )]),
        sync_states,
        FixedClock,
    )
    .with_calendar_view_refresh(RecordingCalendarRefresh::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2026-05-05".to_string(),
                provider: PlannedWorkoutSyncProvider::Intervals,
            },
        )
        .await
        .unwrap();

    assert!(intervals.created_events.lock().unwrap().is_empty());
    let updated = intervals.updated_events.lock().unwrap().clone();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].0, 77);
    assert_eq!(updated[0].1.description.as_deref(), Some("manual note"));
    assert_eq!(
        updated[0].1.workout_doc.as_deref(),
        Some("Stochastic Durability - Over/Unders\nWarmup\n- 15m ramp 175-250W\nMain Set 4x\n- 2m 105%\n- 4m 92%\n- 4m 50%\nCooldown\n- 15m 55%")
    );
}

#[tokio::test]
async fn sync_planned_workout_returns_credentials_not_configured_when_wahoo_is_not_connected() {
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let service = CalendarService::new(
        FakeIntervalsService::with_events_error(IntervalsError::ConnectionError(
            "intervals unused in not-connected Wahoo sync path".to_string(),
        )),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states.clone(),
        FixedClock,
    )
    .with_wahoo(
        NoopWahooUseCases,
        InMemoryUserSettingsRepository::with_ftp(295),
    )
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default());

    let error = service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, CalendarError::CredentialsNotConfigured);
    let wahoo_sync = sync_states
        .find_by_provider_and_canonical_entity(
            "user-1",
            ExternalProvider::Wahoo,
            &planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
        )
        .await
        .unwrap()
        .expect("expected failed wahoo sync record");
    assert_eq!(wahoo_sync.wahoo_plan_id, None);
    assert_eq!(wahoo_sync.wahoo_workout_id, None);
    assert!(wahoo_sync.last_error.is_some());
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push((user_id, oldest, newest));
            Ok(Vec::new())
        })
    }
}

#[derive(Clone)]
struct FakeIntervalsService {
    created_event: Event,
    list_events_error: Option<IntervalsError>,
    created_events: Arc<Mutex<Vec<CreateEvent>>>,
    updated_events: Arc<Mutex<Vec<(i64, UpdateEvent)>>>,
}

impl FakeIntervalsService {
    fn with_created_event(created_event: Event) -> Self {
        Self {
            created_event,
            list_events_error: None,
            created_events: Arc::new(Mutex::new(Vec::new())),
            updated_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_events_error(list_events_error: IntervalsError) -> Self {
        Self {
            created_event: Event {
                id: 0,
                start_date_local: "2026-03-26T00:00:00".to_string(),
                event_type: None,
                name: None,
                category: EventCategory::Workout,
                description: None,
                indoor: false,
                color: None,
                workout_doc: None,
            },
            list_events_error: Some(list_events_error),
            created_events: Arc::new(Mutex::new(Vec::new())),
            updated_events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl IntervalsUseCases for FakeIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> IntervalsBoxFuture<Result<Vec<Event>, IntervalsError>> {
        let list_events_error = self.list_events_error.clone();
        Box::pin(async move {
            match list_events_error {
                Some(error) => Err(error),
                None => Ok(Vec::new()),
            }
        })
    }

    fn get_event(
        &self,
        _user_id: &str,
        event_id: i64,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_event = self.created_event.clone();
        Box::pin(async move {
            if created_event.id == event_id {
                Ok(created_event)
            } else {
                Err(IntervalsError::NotFound)
            }
        })
    }

    fn create_event(
        &self,
        _user_id: &str,
        event: CreateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_event = self.created_event.clone();
        let created_events = self.created_events.clone();
        Box::pin(async move {
            created_events.lock().unwrap().push(event);
            Ok(created_event)
        })
    }

    fn update_event(
        &self,
        _user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_event = self.created_event.clone();
        let updated_events = self.updated_events.clone();
        Box::pin(async move {
            updated_events.lock().unwrap().push((event_id, event));
            Ok(created_event)
        })
    }

    fn delete_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { Ok(()) })
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
struct InMemoryExternalSyncStateRepository {
    stored: Arc<Mutex<Vec<ExternalSyncState>>>,
}

#[derive(Clone)]
struct FixedPlannedWorkoutTokenRepository {
    match_token: String,
}

impl FixedPlannedWorkoutTokenRepository {
    fn new(match_token: &str) -> Self {
        Self {
            match_token: match_token.to_string(),
        }
    }
}

impl crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository
    for FixedPlannedWorkoutTokenRepository
{
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<
            Option<PlannedWorkoutToken>,
            crate::domain::planned_workout_tokens::PlannedWorkoutTokenError,
        >,
    > {
        let match_token = self.match_token.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            Ok(Some(PlannedWorkoutToken::new(
                user_id,
                planned_workout_id,
                match_token,
            )))
        })
    }

    fn upsert(
        &self,
        token: PlannedWorkoutToken,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<
            PlannedWorkoutToken,
            crate::domain::planned_workout_tokens::PlannedWorkoutTokenError,
        >,
    > {
        Box::pin(async move { Ok(token) })
    }

    fn find_by_match_token(
        &self,
        user_id: &str,
        match_token: &str,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<
            Option<PlannedWorkoutToken>,
            crate::domain::planned_workout_tokens::PlannedWorkoutTokenError,
        >,
    > {
        let match_token_value = self.match_token.clone();
        let user_id = user_id.to_string();
        let match_token = match_token.to_string();
        Box::pin(async move {
            Ok((match_token_value == match_token).then(|| {
                PlannedWorkoutToken::new(
                    user_id,
                    "training-plan:user-1:w1:1:2023-11-14".to_string(),
                    match_token_value,
                )
            }))
        })
    }
}

#[tokio::test]
async fn sync_planned_workout_to_wahoo_adds_workout_token() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("- 60m 70%".to_string()),
        indoor: false,
        color: None,
        workout_doc: None,
    });
    let wahoo = RecordingWahooService::successful();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let service = CalendarService::new(
        intervals.clone(),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states,
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), InMemoryUserSettingsRepository::with_ftp(295))
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    let created_workouts = wahoo.created_workouts();
    assert_eq!(created_workouts.len(), 1);
    assert_eq!(created_workouts[0].name, "Build Session");
    assert_eq!(created_workouts[0].starts, "2023-11-14T00:00:00.000Z");
    assert_eq!(created_workouts[0].minutes, 60);
    assert!(created_workouts[0]
        .workout_token
        .starts_with("[AIWATTCOACH:pw="));
}

#[tokio::test]
async fn sync_planned_workout_preserves_single_marker_when_updating_existing_event() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 88,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("Keep this note\n\n[AIWATTCOACH:pw=ABC123EF45]".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc: None,
    });
    let wahoo = RecordingWahooService::with_listed_workouts(vec![WahooWorkout {
        id: 6001,
        starts: "2023-11-14T00:00:00.000Z".to_string(),
        minutes: Some(60),
        name: Some("Build Session".to_string()),
        plan_id: Some(5001),
        plan_ids: vec![5001],
        route_id: None,
        workout_token: Some("[AIWATTCOACH:pw=ABC123EF45]".to_string()),
        workout_type_id: Some(0),
        workout_summary: None,
        created_at: None,
        updated_at: None,
    }]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Intervals,
                planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
            )
            .mark_synced("88".to_string(), "old-hash".to_string(), 1_700_000_001),
        )
        .await
        .unwrap();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Wahoo,
                planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
            )
            .mark_wahoo_pending("training-plan:user-1:w1:1:2023-11-14".to_string())
            .mark_wahoo_synced(
                "old-hash".to_string(),
                1_700_000_001,
                "training-plan:user-1:w1:1:2023-11-14".to_string(),
                5001,
                6001,
                "[AIWATTCOACH:pw=ABC123EF45]".to_string(),
            ),
        )
        .await
        .unwrap();

    let service = CalendarService::new(
        intervals.clone(),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states,
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), InMemoryUserSettingsRepository::with_ftp(295))
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    let updated_workouts = wahoo.updated_workouts();
    assert_eq!(updated_workouts.len(), 1);
    assert_eq!(updated_workouts[0].0, 6001);
    assert_eq!(
        updated_workouts[0].1.starts.as_deref(),
        Some("2023-11-14T00:00:00.000Z")
    );
    assert_eq!(updated_workouts[0].1.name.as_deref(), Some("Build Session"));
    assert_eq!(updated_workouts[0].1.minutes, Some(60));
    assert_eq!(updated_workouts[0].1.plan_id, Some(5001));
    assert_eq!(
        updated_workouts[0].1.workout_token.as_deref(),
        Some("[AIWATTCOACH:pw=ABC123EF45]")
    );
}

#[tokio::test]
async fn sync_planned_workout_to_wahoo_reuses_remote_workout_found_by_token() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("- 60m 70%".to_string()),
        indoor: false,
        color: None,
        workout_doc: None,
    });
    let match_token = crate::domain::planned_workout_tokens::build_planned_workout_match_token(
        "training-plan:user-1:w1:1:2023-11-14",
    );
    let workout_token =
        crate::domain::planned_workout_tokens::format_planned_workout_marker(&match_token);
    let wahoo = RecordingWahooService::with_listed_workouts(vec![WahooWorkout {
        id: 7001,
        starts: "2023-11-14T00:00:00.000Z".to_string(),
        minutes: Some(60),
        name: Some("Build Session".to_string()),
        plan_id: Some(5001),
        plan_ids: vec![5001],
        route_id: None,
        workout_token: Some(workout_token.clone()),
        workout_type_id: Some(0),
        workout_summary: None,
        created_at: None,
        updated_at: None,
    }]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Wahoo,
                planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
            )
            .mark_wahoo_pending("training-plan:user-1:w1:1:2023-11-14".to_string()),
        )
        .await
        .unwrap();

    let service = CalendarService::new(
        intervals,
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states,
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), InMemoryUserSettingsRepository::with_ftp(295))
    .with_planned_workout_tokens(FixedPlannedWorkoutTokenRepository::new(&match_token));

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    assert_eq!(wahoo.workout_create_calls(), 0);
    let updated_workouts = wahoo.updated_workouts();
    assert_eq!(updated_workouts.len(), 1);
    assert_eq!(updated_workouts[0].0, 7001);
}

#[tokio::test]
async fn sync_planned_workout_to_wahoo_reuses_remote_workout_found_by_token_on_later_page() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("- 60m 70%".to_string()),
        indoor: false,
        color: None,
        workout_doc: None,
    });
    let match_token = crate::domain::planned_workout_tokens::build_planned_workout_match_token(
        "training-plan:user-1:w1:1:2023-11-14",
    );
    let workout_token =
        crate::domain::planned_workout_tokens::format_planned_workout_marker(&match_token);
    let mut workouts = (0..100)
        .map(|index| WahooWorkout {
            id: 8_000 + index,
            starts: "2023-11-13T00:00:00.000Z".to_string(),
            minutes: Some(60),
            name: Some(format!("Other Session {index}")),
            plan_id: Some(5_000 + index),
            plan_ids: vec![5_000 + index],
            route_id: None,
            workout_token: Some(format!("[AIWATTCOACH:pw=OTHER{index:05}]")),
            workout_type_id: Some(0),
            workout_summary: None,
            created_at: None,
            updated_at: None,
        })
        .collect::<Vec<_>>();
    workouts.push(WahooWorkout {
        id: 9001,
        starts: "2023-11-14T00:00:00.000Z".to_string(),
        minutes: Some(60),
        name: Some("Build Session".to_string()),
        plan_id: Some(5001),
        plan_ids: vec![5001],
        route_id: None,
        workout_token: Some(workout_token.clone()),
        workout_type_id: Some(0),
        workout_summary: None,
        created_at: None,
        updated_at: None,
    });
    let wahoo = RecordingWahooService::with_listed_workouts(workouts);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Wahoo,
                planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
            )
            .mark_wahoo_pending("training-plan:user-1:w1:1:2023-11-14".to_string()),
        )
        .await
        .unwrap();

    let service = CalendarService::new(
        intervals,
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states,
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), InMemoryUserSettingsRepository::with_ftp(295))
    .with_planned_workout_tokens(FixedPlannedWorkoutTokenRepository::new(&match_token));

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    assert_eq!(wahoo.workout_create_calls(), 0);
    let updated_workouts = wahoo.updated_workouts();
    assert_eq!(updated_workouts.len(), 1);
    assert_eq!(updated_workouts[0].0, 9001);
}

#[tokio::test]
async fn sync_planned_workout_to_wahoo_recreates_workout_after_stale_remote_id_not_found() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2023-11-14T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("- 60m 70%".to_string()),
        indoor: false,
        color: None,
        workout_doc: None,
    });
    let match_token = crate::domain::planned_workout_tokens::build_planned_workout_match_token(
        "training-plan:user-1:w1:1:2023-11-14",
    );
    let workout_token =
        crate::domain::planned_workout_tokens::format_planned_workout_marker(&match_token);
    let wahoo = RecordingWahooService::with_update_workout_not_found(6001);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Wahoo,
                planned_workout_entity("training-plan:user-1:w1:1", "2023-11-14"),
            )
            .mark_wahoo_pending("training-plan:user-1:w1:1:2023-11-14".to_string())
            .mark_wahoo_synced(
                "old-hash".to_string(),
                1_700_000_001,
                "training-plan:user-1:w1:1:2023-11-14".to_string(),
                5001,
                6001,
                workout_token.clone(),
            ),
        )
        .await
        .unwrap();

    let service = CalendarService::new(
        intervals,
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2023-11-14",
            "Build Session",
        )]),
        sync_states,
        FixedClock,
    )
    .with_wahoo(wahoo.clone(), InMemoryUserSettingsRepository::with_ftp(295))
    .with_planned_workout_tokens(FixedPlannedWorkoutTokenRepository::new(&match_token));

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2023-11-14".to_string(),
                provider: PlannedWorkoutSyncProvider::Wahoo,
            },
        )
        .await
        .unwrap();

    let updated_workouts = wahoo.updated_workouts();
    assert_eq!(updated_workouts.len(), 0);
    assert_eq!(wahoo.workout_create_calls(), 1);
}

impl ExternalSyncStateRepository for InMemoryExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.canonical_entity == state.canonical_entity)
            });
            stored.push(state.clone());
            Ok(state)
        })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity == canonical_entity
                })
                .cloned())
        })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.user_id == user_id && state.provider == provider)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
        })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            stored.lock().unwrap().retain(|state| {
                !(state.user_id == user_id
                    && state.provider == provider
                    && state.canonical_entity == canonical_entity)
            });
            Ok(())
        })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_plan_id == Some(wahoo_plan_id)
                })
                .cloned())
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_workout_token.as_deref()
                            == Some(wahoo_workout_token.as_str())
                })
                .cloned())
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity.entity_kind
                            == crate::domain::external_sync::CanonicalEntityKind::PlannedWorkout
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryCalendarEntryViewRepository {
    stored: Arc<Mutex<Vec<CalendarEntryView>>>,
}

impl CalendarEntryViewRepository for InMemoryCalendarEntryViewRepository {
    fn find_oldest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<Result<Option<String>, CalendarEntryViewError>>
    {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .map(|entry| entry.date.clone())
                .min())
        })
    }

    fn find_newest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<Result<Option<String>, CalendarEntryViewError>>
    {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .map(|entry| entry.date.clone())
                .max())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .filter(|entry| entry.date >= oldest && entry.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        entry: CalendarEntryView,
    ) -> crate::domain::calendar_view::BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>>
    {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == entry.user_id && existing.entry_id == entry.entry_id)
            });
            stored.push(entry.clone());
            Ok(entry)
        })
    }

    fn replace_all_for_user(
        &self,
        user_id: &str,
        entries: Vec<CalendarEntryView>,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| existing.user_id != user_id);
            stored.extend(entries.clone());
            Ok(entries)
        })
    }

    fn replace_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        entries: Vec<CalendarEntryView>,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                existing.user_id != user_id || existing.date < oldest || existing.date > newest
            });
            stored.extend(entries.clone());
            Ok(entries)
        })
    }
}

#[derive(Clone, Default)]
struct FakeProjectionRepository {
    days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
}

impl FakeProjectionRepository {
    fn with_days(days: Vec<TrainingPlanProjectedDay>) -> Self {
        Self {
            days: Arc::new(Mutex::new(days)),
        }
    }
}

impl TrainingPlanProjectionRepository for FakeProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
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
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
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
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
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
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> TrainingPlanBoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>> {
        Box::pin(async move {
            Ok(TrainingPlanReplacementResult {
                snapshot,
                projected_days,
                superseded_date_range: None,
            })
        })
    }
}

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

fn planned_workout_entity(operation_key: &str, date: &str) -> CanonicalEntityRef {
    CanonicalEntityRef::new(
        CanonicalEntityKind::PlannedWorkout,
        format!("{operation_key}:{date}"),
    )
}

#[tokio::test]
async fn list_events_reads_from_calendar_entry_view_only() {
    let entries = InMemoryCalendarEntryViewRepository::default();
    entries
        .upsert(CalendarEntryView {
            entry_id: "planned:training-plan:user-1:w1:1:2026-03-26".to_string(),
            user_id: "user-1".to_string(),
            entry_kind: CalendarEntryKind::PlannedWorkout,
            date: "2026-03-26".to_string(),
            start_date_local: Some("2026-03-26T00:00:00".to_string()),
            title: "Build Session".to_string(),
            subtitle: Some("2 lines".to_string()),
            description: None,
            rest_day: false,
            rest_day_reason: None,
            raw_workout_doc: Some("Build Session\n- 60m 70%".to_string()),
            planned_workout_id: Some("training-plan:user-1:w1:1:2026-03-26".to_string()),
            completed_workout_id: None,
            race_id: None,
            special_day_id: None,
            race: None,
            summary: None,
            sync: Some(CalendarEntrySync {
                linked_intervals_event_id: Some(77),
                sync_status: Some("synced".to_string()),
            }),
        })
        .await
        .unwrap();

    let service = CalendarService::new(
        FakeIntervalsService::with_events_error(IntervalsError::ConnectionError(
            "should not be called".to_string(),
        )),
        entries,
        FakeProjectionRepository::default(),
        InMemoryExternalSyncStateRepository::default(),
        FixedClock,
    )
    .with_completed_workouts(InMemoryCompletedWorkoutRepository::default());

    let events = service
        .list_events(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 77);
    assert_eq!(
        events[0].calendar_entry_id,
        "planned:training-plan:user-1:w1:1:2026-03-26"
    );
    assert_eq!(events[0].name.as_deref(), Some("Build Session"));
    assert_eq!(events[0].start_date_local, "2026-03-26");
    assert_eq!(
        events[0].raw_workout_doc.as_deref(),
        Some("Build Session\n- 60m 70%")
    );
}

#[tokio::test]
async fn list_events_skips_completed_entries_even_with_planned_backlink() {
    let entries = InMemoryCalendarEntryViewRepository::default();
    entries
        .upsert(CalendarEntryView {
            entry_id: "completed:completed-1".to_string(),
            user_id: "user-1".to_string(),
            entry_kind: CalendarEntryKind::CompletedWorkout,
            date: "2026-03-26".to_string(),
            start_date_local: Some("2026-03-26T08:00:00".to_string()),
            title: "Completed Build Session".to_string(),
            subtitle: Some("TSS 82".to_string()),
            description: Some("Strong day".to_string()),
            rest_day: false,
            rest_day_reason: None,
            raw_workout_doc: None,
            planned_workout_id: Some("training-plan:user-1:w1:1:2026-03-26".to_string()),
            completed_workout_id: Some("completed-1".to_string()),
            race_id: None,
            special_day_id: None,
            race: None,
            summary: None,
            sync: None,
        })
        .await
        .unwrap();

    let service = CalendarService::new(
        FakeIntervalsService::with_created_event(Event {
            id: 0,
            start_date_local: "2026-03-26T00:00:00".to_string(),
            event_type: None,
            name: None,
            category: EventCategory::Workout,
            description: None,
            indoor: false,
            color: None,
            workout_doc: None,
        }),
        entries,
        FakeProjectionRepository::default(),
        InMemoryExternalSyncStateRepository::default(),
        FixedClock,
    )
    .with_completed_workouts(InMemoryCompletedWorkoutRepository::default());

    let events = service
        .list_events(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(events.is_empty());
}

#[tokio::test]
async fn list_events_hydrates_actual_workout_from_linked_completed_workout() {
    let entries = InMemoryCalendarEntryViewRepository::default();
    entries
        .upsert(CalendarEntryView {
            entry_id: "planned:training-plan:user-1:w1:1:2026-03-26".to_string(),
            user_id: "user-1".to_string(),
            entry_kind: CalendarEntryKind::PlannedWorkout,
            date: "2026-03-26".to_string(),
            start_date_local: Some("2026-03-26T00:00:00".to_string()),
            title: "Build Session".to_string(),
            subtitle: Some("2 lines".to_string()),
            description: None,
            rest_day: false,
            rest_day_reason: None,
            raw_workout_doc: Some("Build Session\n- 60m 70%".to_string()),
            planned_workout_id: Some("training-plan:user-1:w1:1:2026-03-26".to_string()),
            completed_workout_id: Some("intervals-activity:a41".to_string()),
            race_id: None,
            special_day_id: None,
            race: None,
            summary: None,
            sync: Some(CalendarEntrySync {
                linked_intervals_event_id: Some(77),
                sync_status: Some("synced".to_string()),
            }),
        })
        .await
        .unwrap();
    let completed = InMemoryCompletedWorkoutRepository::default();
    completed
        .upsert(sample_completed_workout("intervals-activity:a41"))
        .await
        .unwrap();

    let service = CalendarService::new(
        FakeIntervalsService::with_events_error(IntervalsError::ConnectionError(
            "should not be called".to_string(),
        )),
        entries,
        FakeProjectionRepository::default(),
        InMemoryExternalSyncStateRepository::default(),
        FixedClock,
    )
    .with_completed_workouts(completed);

    let events = service
        .list_events(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    let actual = events[0].actual_workout.as_ref().expect("actual workout");
    assert_eq!(actual.activity_id, "intervals-activity:a41");
    assert_eq!(
        actual.activity_name.as_deref(),
        Some("Completed Build Session")
    );
    assert_eq!(actual.training_stress_score, Some(82));
    assert_eq!(actual.power_values, vec![180, 240, 310]);
}

#[derive(Clone, Default)]
struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Option<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> crate::domain::completed_workouts::BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>>
    {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryUserSettingsRepository {
    stored: Arc<Mutex<Vec<UserSettings>>>,
}

impl InMemoryUserSettingsRepository {
    fn with_ftp(ftp_watts: u32) -> Self {
        Self {
            stored: Arc::new(Mutex::new(vec![UserSettings {
                user_id: "user-1".to_string(),
                intervals: Default::default(),
                wahoo: WahooConfig {
                    connected: true,
                    ..Default::default()
                },
                ai_agents: Default::default(),
                options: Default::default(),
                cycling: CyclingSettings {
                    ftp_watts: Some(ftp_watts),
                    ..Default::default()
                },
                availability: Default::default(),
                created_at_epoch_seconds: 1_700_000_000,
                updated_at_epoch_seconds: 1_700_000_000,
            }])),
        }
    }
}

impl UserSettingsRepository for InMemoryUserSettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|settings| settings.user_id == user_id)
                .cloned())
        })
    }

    fn find_by_wahoo_user_id(
        &self,
        wahoo_user_id: i64,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|settings| settings.wahoo.user_id == Some(wahoo_user_id))
                .cloned())
        })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> crate::domain::settings::BoxFuture<
        Result<Vec<crate::domain::settings::WahooUserIdBackfillCandidate>, SettingsError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|settings| {
                    settings.wahoo.connected
                        && settings.wahoo.user_id.is_none()
                        && settings
                            .wahoo
                            .refresh_token
                            .as_deref()
                            .is_some_and(|value| !value.trim().is_empty())
                })
                .cloned()
                .map(
                    |settings| crate::domain::settings::WahooUserIdBackfillCandidate {
                        user_id: settings.user_id,
                        wahoo: settings.wahoo,
                    },
                )
                .collect())
        })
    }

    fn upsert(
        &self,
        settings: UserSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| existing.user_id != settings.user_id);
            stored.push(settings.clone());
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: crate::domain::settings::AiAgentsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: crate::domain::settings::IntervalsConfig,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: crate::domain::settings::AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: crate::domain::settings::AvailabilitySettings,
        _updated_at_epoch_seconds: i64,
    ) -> crate::domain::settings::BoxFuture<Result<(), SettingsError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Default)]
struct RecordingWahooService {
    plan_create_calls: Arc<Mutex<usize>>,
    workout_create_calls: Arc<Mutex<usize>>,
    created_workouts: Arc<Mutex<Vec<WahooCreateWorkout>>>,
    updated_workouts: Arc<Mutex<Vec<(i64, WahooUpdateWorkout)>>>,
    listed_workouts: Arc<Mutex<Vec<WahooWorkout>>>,
    failure: Option<String>,
    workout_update_not_found_ids: Arc<Mutex<Vec<i64>>>,
}

impl RecordingWahooService {
    #[allow(dead_code)]
    fn successful() -> Self {
        Self::default()
    }

    fn failing(message: &str) -> Self {
        Self {
            failure: Some(message.to_string()),
            ..Self::default()
        }
    }

    fn plan_create_calls(&self) -> usize {
        *self.plan_create_calls.lock().unwrap()
    }

    fn workout_create_calls(&self) -> usize {
        *self.workout_create_calls.lock().unwrap()
    }

    fn created_workouts(&self) -> Vec<WahooCreateWorkout> {
        self.created_workouts.lock().unwrap().clone()
    }

    fn updated_workouts(&self) -> Vec<(i64, WahooUpdateWorkout)> {
        self.updated_workouts.lock().unwrap().clone()
    }

    fn with_listed_workouts(workouts: Vec<WahooWorkout>) -> Self {
        Self {
            listed_workouts: Arc::new(Mutex::new(workouts)),
            ..Self::default()
        }
    }

    fn with_update_workout_not_found(workout_id: i64) -> Self {
        Self {
            workout_update_not_found_ids: Arc::new(Mutex::new(vec![workout_id])),
            ..Self::default()
        }
    }
}

impl WahooUseCases for RecordingWahooService {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> crate::domain::wahoo::BoxFuture<Result<crate::domain::wahoo::WahooAuthStart, WahooError>>
    {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<crate::domain::wahoo::WahooAuthExchange, WahooError>>
    {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn ensure_token(
        &self,
        _user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<crate::domain::wahoo::WahooToken, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_authenticated_user(
        &self,
        _user_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<crate::domain::wahoo::WahooUser, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        page: usize,
        per_page: usize,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkoutList, WahooError>> {
        let listed_workouts = self.listed_workouts.clone();
        Box::pin(async move {
            let workouts = listed_workouts.lock().unwrap().clone();
            let start = per_page.saturating_mul(page.saturating_sub(1));
            let page_workouts = workouts
                .iter()
                .skip(start)
                .take(per_page)
                .cloned()
                .collect::<Vec<_>>();
            Ok(WahooWorkoutList {
                total: workouts.len(),
                workouts: page_workouts,
                page,
                per_page,
                order: None,
                sort: None,
            })
        })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotFound) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { Ok(None) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        request: WahooCreatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        let calls = self.plan_create_calls.clone();
        let failure = self.failure.clone();
        Box::pin(async move {
            *calls.lock().unwrap() += 1;
            if let Some(message) = failure {
                return Err(WahooError::External(message));
            }
            Ok(WahooPlan {
                id: 5001,
                external_id: request.external_id,
                provider_updated_at: Some(request.provider_updated_at),
                filename: request.filename,
                name: None,
                description: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooPlan, WahooError>> {
        let failure = self.failure.clone();
        Box::pin(async move {
            if let Some(message) = failure {
                return Err(WahooError::External(message));
            }
            Ok(WahooPlan {
                id: plan_id,
                external_id: "training-plan:user-1:w1:1:2026-03-26".to_string(),
                provider_updated_at: Some(request.provider_updated_at),
                filename: request.filename,
                name: None,
                description: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        request: WahooCreateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        let calls = self.workout_create_calls.clone();
        let created_workouts = self.created_workouts.clone();
        let failure = self.failure.clone();
        Box::pin(async move {
            *calls.lock().unwrap() += 1;
            created_workouts.lock().unwrap().push(request.clone());
            if let Some(message) = failure {
                return Err(WahooError::External(message));
            }
            Ok(WahooWorkout {
                id: 6001,
                starts: request.starts,
                minutes: Some(request.minutes),
                name: Some(request.name),
                plan_id: request.plan_id,
                plan_ids: request.plan_id.into_iter().collect(),
                route_id: None,
                workout_token: Some(request.workout_token),
                workout_type_id: Some(request.workout_type_id),
                workout_summary: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> crate::domain::wahoo::BoxFuture<Result<WahooWorkout, WahooError>> {
        let updated_workouts = self.updated_workouts.clone();
        let failure = self.failure.clone();
        let workout_update_not_found_ids = self.workout_update_not_found_ids.clone();
        Box::pin(async move {
            updated_workouts
                .lock()
                .unwrap()
                .push((workout_id, request.clone()));
            if workout_update_not_found_ids
                .lock()
                .unwrap()
                .contains(&workout_id)
            {
                return Err(WahooError::NotFound);
            }
            if let Some(message) = failure {
                return Err(WahooError::External(message));
            }
            Ok(WahooWorkout {
                id: workout_id,
                starts: request
                    .starts
                    .unwrap_or_else(|| "2026-03-26T00:00:00.000Z".to_string()),
                minutes: request.minutes,
                name: request.name,
                plan_id: request.plan_id,
                plan_ids: request.plan_id.into_iter().collect(),
                route_id: None,
                workout_token: request.workout_token,
                workout_type_id: request.workout_type_id,
                workout_summary: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> crate::domain::wahoo::BoxFuture<Result<Vec<u8>, WahooError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

fn sample_completed_workout(completed_workout_id: &str) -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: completed_workout_id.to_string(),
        user_id: "user-1".to_string(),
        start_date_local: "2026-03-26T08:00:00".to_string(),
        source_activity_id: Some(completed_workout_id.to_string()),
        planned_workout_id: Some("training-plan:user-1:w1:1:2026-03-26".to_string()),
        name: Some("Completed Build Session".to_string()),
        description: Some("Strong day".to_string()),
        activity_type: Some("Ride".to_string()),
        external_id: Some("external-completed".to_string()),
        trainer: false,
        duration_seconds: Some(3600),
        distance_meters: Some(35200.0),
        metrics: CompletedWorkoutMetrics {
            training_stress_score: Some(82),
            normalized_power_watts: Some(252),
            intensity_factor: Some(0.86),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(228),
            ftp_watts: Some(295),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: crate::domain::completed_workouts::CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
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
    }
}

fn projected_day(
    user_id: &str,
    operation_key: &str,
    date: &str,
    workout_name: &str,
) -> TrainingPlanProjectedDay {
    projected_day_with_doc(
        user_id,
        operation_key,
        date,
        &format!("{workout_name}\n- 60m 70%"),
    )
}

fn projected_day_with_doc(
    user_id: &str,
    operation_key: &str,
    date: &str,
    workout_doc: &str,
) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: user_id.to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: operation_key.to_string(),
        date: date.to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: Some(parse_planned_workout(workout_doc).expect("planned workout should parse")),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}
