use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar::{
        CalendarError, CalendarService, CalendarUseCases, HiddenCalendarEventSource,
        PlannedWorkoutSyncRecord, PlannedWorkoutSyncRepository, SyncPlannedWorkout,
    },
    calendar_view::{CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort},
    identity::Clock,
    intervals::{
        parse_planned_workout, BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event,
        EventCategory, IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    training_plan::{
        BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
        TrainingPlanProjectionRepository, TrainingPlanSnapshot,
    },
};

#[tokio::test]
async fn sync_planned_workout_refreshes_calendar_view_for_synced_day() {
    let refresh = RecordingCalendarRefresh::default();
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
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryPlannedWorkoutSyncRepository::default(),
        EmptyHiddenCalendarEventSource,
        FixedClock,
    )
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
    let service = CalendarService::new(
        FakeIntervalsService::with_create_error(IntervalsError::ConnectionError(
            "intervals unavailable".to_string(),
        )),
        FakeProjectionRepository::with_days(vec![projected_day(
            "user-1",
            "training-plan:user-1:w1:1",
            "2026-03-26",
            "Build Session",
        )]),
        InMemoryPlannedWorkoutSyncRepository::default(),
        EmptyHiddenCalendarEventSource,
        FixedClock,
    )
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
}

impl FakeIntervalsService {
    fn with_created_event(created_event: Event) -> Self {
        Self {
            created_event,
            create_error: None,
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
        }
    }
}

impl IntervalsUseCases for FakeIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> IntervalsBoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn create_event(
        &self,
        _user_id: &str,
        _event: CreateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_event = self.created_event.clone();
        let create_error = self.create_error.clone();
        Box::pin(async move {
            match create_error {
                Some(error) => Err(error),
                None => Ok(created_event),
            }
        })
    }

    fn update_event(
        &self,
        _user_id: &str,
        _event_id: i64,
        _event: UpdateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_event = self.created_event.clone();
        Box::pin(async move { Ok(created_event) })
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
struct EmptyHiddenCalendarEventSource;

impl HiddenCalendarEventSource for EmptyHiddenCalendarEventSource {
    fn list_hidden_intervals_event_ids(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> crate::domain::calendar::BoxFuture<Result<Vec<i64>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
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
        workout: Some(
            parse_planned_workout(&format!("{workout_name}\n- 60m 70%"))
                .expect("planned workout should parse"),
        ),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}
