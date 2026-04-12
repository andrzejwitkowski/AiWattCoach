use super::service::{validate_request, RaceService};
use crate::domain::{
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository, ExternalSyncStatus,
    },
    identity::{Clock, IdGenerator},
    intervals::{
        BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event, EventCategory,
        IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    races::{
        BoxFuture, CreateRace, Race, RaceDiscipline, RaceError, RacePriority, RaceRepository,
        RaceUseCases, UpdateRace,
    },
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
struct TestIdGenerator;

static TEST_ID_COUNTER: AtomicUsize = AtomicUsize::new(123);

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, _prefix: &str) -> String {
        format!("race-{}", TEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Default)]
struct InMemoryRaceRepository {
    races: Arc<Mutex<Vec<Race>>>,
}

impl InMemoryRaceRepository {
    fn with_races(races: Vec<Race>) -> Self {
        Self {
            races: Arc::new(Mutex::new(races)),
        }
    }

    fn stored(&self) -> Vec<Race> {
        self.races.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
struct InMemoryExternalSyncStateRepository {
    states: Arc<Mutex<Vec<ExternalSyncState>>>,
}

impl InMemoryExternalSyncStateRepository {
    fn stored(&self) -> Vec<ExternalSyncState> {
        self.states.lock().unwrap().clone()
    }
}

impl ExternalSyncStateRepository for InMemoryExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<Result<ExternalSyncState, std::convert::Infallible>>
    {
        let states = self.states.clone();
        Box::pin(async move {
            let mut states = states.lock().unwrap();
            states.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.canonical_entity == state.canonical_entity)
            });
            states.push(state.clone());
            Ok(state)
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, std::convert::Infallible>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            Ok(states
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

    fn delete_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), std::convert::Infallible>> {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            states.lock().unwrap().retain(|state| {
                !(state.user_id == user_id
                    && state.provider == provider
                    && state.canonical_entity == canonical_entity)
            });
            Ok(())
        })
    }
}

impl RaceRepository for InMemoryRaceRepository {
    fn list_by_user_id(&self, user_id: &str) -> BoxFuture<Result<Vec<Race>, RaceError>> {
        let races = self.races.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(races
                .lock()
                .unwrap()
                .iter()
                .filter(|race| race.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Race>, RaceError>> {
        let races = self.races.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(races
                .lock()
                .unwrap()
                .iter()
                .filter(|race| race.user_id == user_id)
                .filter(|race| race.date >= oldest && race.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> BoxFuture<Result<Option<Race>, RaceError>> {
        let races = self.races.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            Ok(races
                .lock()
                .unwrap()
                .iter()
                .find(|race| race.user_id == user_id && race.race_id == race_id)
                .cloned())
        })
    }

    fn upsert(&self, race: Race) -> BoxFuture<Result<Race, RaceError>> {
        let races = self.races.clone();
        Box::pin(async move {
            let mut races = races.lock().unwrap();
            races.retain(|existing| {
                !(existing.user_id == race.user_id && existing.race_id == race.race_id)
            });
            races.push(race.clone());
            Ok(race)
        })
    }

    fn delete(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<(), RaceError>> {
        let races = self.races.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            races
                .lock()
                .unwrap()
                .retain(|race| !(race.user_id == user_id && race.race_id == race_id));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct RecordingIntervalsService {
    created_events: Arc<Mutex<Vec<CreateEvent>>>,
    updated_events: Arc<Mutex<Vec<(i64, UpdateEvent)>>>,
    deleted_event_ids: Arc<Mutex<Vec<i64>>>,
    fail_updates: bool,
}

impl RecordingIntervalsService {
    fn with_failed_updates() -> Self {
        Self {
            fail_updates: true,
            ..Self::default()
        }
    }
}

impl IntervalsUseCases for RecordingIntervalsService {
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
        event: CreateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_events = self.created_events.clone();
        Box::pin(async move {
            created_events.lock().unwrap().push(event.clone());
            Ok(Event {
                id: 77,
                start_date_local: event.start_date_local,
                event_type: event.event_type,
                name: event.name,
                category: event.category,
                description: event.description,
                indoor: event.indoor,
                color: event.color,
                workout_doc: event.workout_doc,
            })
        })
    }

    fn update_event(
        &self,
        _user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let updated_events = self.updated_events.clone();
        let fail_updates = self.fail_updates;
        Box::pin(async move {
            if fail_updates {
                return Err(IntervalsError::ConnectionError("boom".to_string()));
            }
            updated_events
                .lock()
                .unwrap()
                .push((event_id, event.clone()));
            Ok(Event {
                id: event_id,
                start_date_local: event
                    .start_date_local
                    .unwrap_or_else(|| "2026-09-12T00:00:00".to_string()),
                event_type: event.event_type,
                name: event.name,
                category: event.category.unwrap_or(EventCategory::Race),
                description: event.description,
                indoor: event.indoor.unwrap_or(false),
                color: event.color,
                workout_doc: event.workout_doc,
            })
        })
    }

    fn delete_event(
        &self,
        _user_id: &str,
        event_id: i64,
    ) -> IntervalsBoxFuture<Result<(), IntervalsError>> {
        let deleted_event_ids = self.deleted_event_ids.clone();
        Box::pin(async move {
            deleted_event_ids.lock().unwrap().push(event_id);
            Ok(())
        })
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[tokio::test]
async fn create_race_persists_and_syncs_to_intervals() {
    let repository = InMemoryRaceRepository::default();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator,
    );

    let created = service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-12".to_string(),
                name: "Gravel Attack".to_string(),
                distance_meters: 120_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::B,
            },
        )
        .await
        .unwrap();

    assert_eq!(created.race_id, "race-123");
    let created_events = intervals.created_events.lock().unwrap();
    assert_eq!(created_events.len(), 1);
    assert_eq!(created_events[0].category, EventCategory::RaceB);
    assert_eq!(repository.stored().len(), 1);
    let sync_state = sync_states.stored().pop().expect("expected sync state");
    assert_eq!(sync_state.external_id.as_deref(), Some("77"));
    assert_eq!(sync_state.sync_status, ExternalSyncStatus::Synced);
}

#[tokio::test]
async fn priority_race_creates_matching_intervals_categories() {
    let repository = InMemoryRaceRepository::default();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states,
        TestClock,
        TestIdGenerator,
    );

    service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-12".to_string(),
                name: "Peak A Race".to_string(),
                distance_meters: 140_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap();

    service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-19".to_string(),
                name: "Support C Race".to_string(),
                distance_meters: 80_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::C,
            },
        )
        .await
        .unwrap();

    let created_events = intervals.created_events.lock().unwrap();
    assert_eq!(created_events.len(), 2);
    assert_eq!(created_events[0].category, EventCategory::RaceA);
    assert_eq!(created_events[1].category, EventCategory::RaceC);
}

#[tokio::test]
async fn update_race_marks_failure_when_intervals_update_fails() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Old Name".to_string(),
        distance_meters: 100_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::C,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    let existing_sync = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        race_ref.clone(),
    )
    .mark_synced("55".to_string(), "old-hash".to_string(), 2);
    sync_states
        .upsert(existing_sync)
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::with_failed_updates();
    let service = RaceService::new(
        repository.clone(),
        intervals,
        sync_states.clone(),
        TestClock,
        TestIdGenerator,
    );

    let error = service
        .update_race(
            "user-1",
            "race-1",
            UpdateRace {
                date: "2026-09-13".to_string(),
                name: "New Name".to_string(),
                distance_meters: 130_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, RaceError::Unavailable("boom".to_string()));
    let stored = repository.stored();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].name, "New Name");
    let updated_sync = sync_states.stored().pop().expect("expected sync state");
    assert_eq!(updated_sync.sync_status, ExternalSyncStatus::Failed);
    assert_eq!(updated_sync.last_error.as_deref(), Some("boom"));
    assert_eq!(updated_sync.canonical_entity, race_ref);
}

#[tokio::test]
async fn delete_race_deletes_remote_event_before_local_remove() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    sync_states
        .upsert(
            ExternalSyncState::new("user-1".to_string(), ExternalProvider::Intervals, race_ref)
                .mark_synced("88".to_string(), "hash".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator,
    );

    service.delete_race("user-1", "race-1").await.unwrap();

    assert!(repository.stored().is_empty());
    assert_eq!(*intervals.deleted_event_ids.lock().unwrap(), vec![88]);
    assert!(sync_states.stored().is_empty());
}

#[test]
fn validate_request_rejects_distance_above_upper_bound() {
    let err = validate_request("2026-09-12", "Race", 10_000_001).unwrap_err();
    assert!(matches!(err, RaceError::Validation(_)));
}

#[test]
fn validate_request_rejects_invalid_date_format() {
    for bad_date in [
        "2026-9-1",
        "26-09-12",
        "2026/09/12",
        "not-a-date",
        "2026-13-01",
        "2026-02-30",
    ] {
        let err = validate_request(bad_date, "Race", 100_000).unwrap_err();
        assert!(
            matches!(err, RaceError::Validation(_)),
            "expected Validation error for date {bad_date:?}, got {err:?}"
        );
    }
}

#[test]
fn validate_request_accepts_valid_date() {
    validate_request("2026-09-12", "Race", 100_000).expect("should accept valid date");
}
