use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar::{
        CalendarError, CalendarService, CalendarUseCases, PlannedWorkoutSyncRecord,
        PlannedWorkoutSyncRepository, SyncPlannedWorkout,
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
        ExternalProvider, ExternalSyncRepositoryError, ProviderPollState,
        ProviderPollStateRepository, ProviderPollStream,
    },
    identity::Clock,
    intervals::{
        parse_planned_workout, BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event,
        EventCategory, IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    planned_workout_tokens::NoopPlannedWorkoutTokenRepository,
    training_plan::{
        BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
        TrainingPlanProjectionRepository, TrainingPlanSnapshot,
    },
};

#[tokio::test]
async fn sync_planned_workout_refreshes_calendar_view_for_synced_day() {
    let refresh = RecordingCalendarRefresh::default();
    let poll_states = InMemoryProviderPollStateRepository::default();
    let service = CalendarService::new(
        FakeIntervalsService::with_created_event(Event {
            id: 77,
            start_date_local: "2026-03-26T00:00:00".to_string(),
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
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryPlannedWorkoutSyncRepository::default(),
        FixedClock,
    )
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default())
    .with_provider_poll_states(poll_states.clone())
    .with_calendar_view_refresh(refresh.clone());

    let result = service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.linked_intervals_event_id, Some(77));
    let poll_state = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .expect("expected poll state");
    assert_eq!(poll_state.next_due_at_epoch_seconds, 1_700_000_000);
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-03-26".to_string(),
            "2026-03-26".to_string(),
        )]
    );
}

#[tokio::test]
async fn sync_planned_workout_refreshes_calendar_view_for_failed_day_after_persisting_failure() {
    let refresh = RecordingCalendarRefresh::default();
    let poll_states = InMemoryProviderPollStateRepository::default();
    let service = CalendarService::new(
        FakeIntervalsService::with_create_error(IntervalsError::ConnectionError(
            "intervals unavailable".to_string(),
        )),
        InMemoryCalendarEntryViewRepository::default(),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryPlannedWorkoutSyncRepository::default(),
        FixedClock,
    )
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default())
    .with_provider_poll_states(poll_states.clone())
    .with_calendar_view_refresh(refresh.clone());

    let error = service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        CalendarError::Unavailable("intervals unavailable".to_string())
    );
    assert!(poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar
        )
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-03-26".to_string(),
            "2026-03-26".to_string(),
        )]
    );
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
    create_error: Option<IntervalsError>,
    list_events_error: Option<IntervalsError>,
    created_events: Arc<Mutex<Vec<CreateEvent>>>,
    updated_events: Arc<Mutex<Vec<(i64, UpdateEvent)>>>,
}

impl FakeIntervalsService {
    fn with_created_event(created_event: Event) -> Self {
        Self {
            created_event,
            create_error: None,
            list_events_error: None,
            created_events: Arc::new(Mutex::new(Vec::new())),
            updated_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_create_error(create_error: IntervalsError) -> Self {
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
            create_error: Some(create_error),
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
            create_error: None,
            list_events_error: Some(list_events_error),
            created_events: Arc::new(Mutex::new(Vec::new())),
            updated_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn created_events(&self) -> Vec<CreateEvent> {
        self.created_events.lock().unwrap().clone()
    }

    fn updated_events(&self) -> Vec<(i64, UpdateEvent)> {
        self.updated_events.lock().unwrap().clone()
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
        let create_error = self.create_error.clone();
        let created_events = self.created_events.clone();
        Box::pin(async move {
            created_events.lock().unwrap().push(event);
            match create_error {
                Some(error) => Err(error),
                None => Ok(created_event),
            }
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
struct InMemoryPlannedWorkoutSyncRepository {
    stored: Arc<Mutex<Vec<PlannedWorkoutSyncRecord>>>,
}

#[tokio::test]
async fn sync_planned_workout_adds_match_marker_to_created_event_description() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 77,
        start_date_local: "2026-03-26T00:00:00".to_string(),
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
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryPlannedWorkoutSyncRepository::default(),
        FixedClock,
    )
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap();

    let created_events = intervals.created_events();
    assert_eq!(created_events.len(), 1);
    let description = created_events[0]
        .description
        .as_deref()
        .expect("created description");
    assert!(description.contains("- 60m 70%"));
    assert!(description.contains("[AIWATTCOACH:pw="));
}

#[tokio::test]
async fn sync_planned_workout_preserves_single_marker_when_updating_existing_event() {
    let intervals = FakeIntervalsService::with_created_event(Event {
        id: 88,
        start_date_local: "2026-03-26T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Build Session".to_string()),
        category: EventCategory::Workout,
        description: Some("Keep this note\n\n[AIWATTCOACH:pw=ABC123EF45]".to_string()),
        indoor: false,
        color: Some("blue".to_string()),
        workout_doc: None,
    });
    let syncs = InMemoryPlannedWorkoutSyncRepository::default();
    syncs
        .upsert(
            PlannedWorkoutSyncRecord::pending(
                "user-1".to_string(),
                "training-plan:user-1:w1:1".to_string(),
                "2026-03-26".to_string(),
                "training-plan:user-1:w1:1".to_string(),
                1_700_000_000,
            )
            .mark_synced(
                88,
                "training-plan:user-1:w1:1".to_string(),
                "old-hash".to_string(),
                1_700_000_001,
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
            "2026-03-26",
            "Build Session",
        )]),
        syncs,
        FixedClock,
    )
    .with_planned_workout_tokens(NoopPlannedWorkoutTokenRepository::default());

    service
        .sync_planned_workout(
            "user-1",
            SyncPlannedWorkout {
                operation_key: "training-plan:user-1:w1:1".to_string(),
                date: "2026-03-26".to_string(),
            },
        )
        .await
        .unwrap();

    let updated_events = intervals.updated_events();
    assert_eq!(updated_events.len(), 1);
    let description = updated_events[0]
        .1
        .description
        .as_deref()
        .expect("updated description");
    assert!(description.contains("Keep this note"));
    assert_eq!(description.matches("[AIWATTCOACH:pw=").count(), 1);
}

impl PlannedWorkoutSyncRepository for InMemoryPlannedWorkoutSyncRepository {
    fn find_by_user_id_and_projection(
        &self,
        user_id: &str,
        operation_key: &str,
        date: &str,
    ) -> crate::domain::calendar::BoxFuture<Result<Option<PlannedWorkoutSyncRecord>, CalendarError>>
    {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let date = date.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|record| {
                    record.user_id == user_id
                        && record.operation_key == operation_key
                        && record.date == date
                })
                .cloned())
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> crate::domain::calendar::BoxFuture<Result<Vec<PlannedWorkoutSyncRecord>, CalendarError>>
    {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|record| record.user_id == user_id)
                .filter(|record| record.date >= oldest && record.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> crate::domain::calendar::BoxFuture<Result<PlannedWorkoutSyncRecord, CalendarError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == record.user_id
                    && existing.operation_key == record.operation_key
                    && existing.date == record.date)
            });
            stored.push(record.clone());
            Ok(record)
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryProviderPollStateRepository {
    stored: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl ProviderPollStateRepository for InMemoryProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ProviderPollState, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.stream == state.stream)
            });
            stored.push(state.clone());
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ProviderPollState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.is_due(now_epoch_seconds))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ProviderPollState>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id && state.provider == provider && state.stream == stream
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
    ) -> TrainingPlanBoxFuture<
        Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
    > {
        Box::pin(async move { Ok((snapshot, projected_days)) })
    }
}

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
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
        InMemoryPlannedWorkoutSyncRepository::default(),
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
        InMemoryPlannedWorkoutSyncRepository::default(),
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
        InMemoryPlannedWorkoutSyncRepository::default(),
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
    TrainingPlanProjectedDay {
        user_id: user_id.to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: operation_key.to_string(),
        date: date.to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: Some(
            parse_planned_workout(&format!("{workout_name}\n- 60m 70%"))
                .expect("planned workout should parse"),
        ),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}
