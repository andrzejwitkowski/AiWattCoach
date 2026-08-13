use crate::domain::{
    calendar_view::{
        CalendarEntryKind, CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
    },
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository, ExternalSyncStatus, ProviderPollState,
        ProviderPollStateRepository, ProviderPollStream,
    },
    identity::{Clock, IdGenerator},
    intervals::{
        BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event, EventCategory,
        IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    races::{BoxFuture, Race, RaceError, RaceRepository},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub(super) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(super) struct TestIdGenerator {
    counter: Arc<AtomicUsize>,
}

impl Default for TestIdGenerator {
    fn default() -> Self {
        Self {
            counter: Arc::new(AtomicUsize::new(123)),
        }
    }
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, _prefix: &str) -> String {
        format!("race-{}", self.counter.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryRaceRepository {
    races: Arc<Mutex<Vec<Race>>>,
    delete_error: Option<RaceError>,
    delete_error_race_id: Option<String>,
}

impl InMemoryRaceRepository {
    pub(super) fn with_races(races: Vec<Race>) -> Self {
        Self {
            races: Arc::new(Mutex::new(races)),
            delete_error: None,
            delete_error_race_id: None,
        }
    }

    pub(super) fn with_delete_error(mut self, error: RaceError) -> Self {
        self.delete_error = Some(error);
        self.delete_error_race_id = None;
        self
    }

    pub(super) fn with_delete_error_for(
        mut self,
        race_id: impl Into<String>,
        error: RaceError,
    ) -> Self {
        self.delete_error = Some(error);
        self.delete_error_race_id = Some(race_id.into());
        self
    }

    pub(super) fn stored(&self) -> Vec<Race> {
        self.races.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryExternalSyncStateRepository {
    states: Arc<Mutex<Vec<ExternalSyncState>>>,
    drop_synced_writes: bool,
    delete_error: Option<ExternalSyncRepositoryError>,
}

impl InMemoryExternalSyncStateRepository {
    pub(super) fn with_dropped_synced_writes() -> Self {
        Self {
            states: Arc::new(Mutex::new(Vec::new())),
            drop_synced_writes: true,
            delete_error: None,
        }
    }

    pub(super) fn with_delete_error(error: ExternalSyncRepositoryError) -> Self {
        Self {
            states: Arc::new(Mutex::new(Vec::new())),
            drop_synced_writes: false,
            delete_error: Some(error),
        }
    }

    pub(super) fn stored(&self) -> Vec<ExternalSyncState> {
        self.states.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryProviderPollStateRepository {
    states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl InMemoryProviderPollStateRepository {
    pub(super) fn stored(&self) -> Vec<ProviderPollState> {
        self.states.lock().unwrap().clone()
    }
}

impl ExternalSyncStateRepository for InMemoryExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let drop_synced_writes = self.drop_synced_writes;
        Box::pin(async move {
            if drop_synced_writes && state.sync_status == ExternalSyncStatus::Synced {
                return Ok(state);
            }
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

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(states
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

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && canonical_entities.contains(&state.canonical_entity)
                })
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
        let states = self.states.clone();
        let delete_error = self.delete_error.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            if let Some(error) = delete_error {
                return Err(error);
            }
            states.lock().unwrap().retain(|state| {
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
        let states = self.states.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(states
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
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            Ok(states
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

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity.entity_kind == CanonicalEntityKind::PlannedWorkout
                        && state.external_id.as_deref() == Some(external_id.as_str())
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
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(states
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
}

impl ProviderPollStateRepository for InMemoryProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ProviderPollState, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        Box::pin(async move {
            let mut states = states.lock().unwrap();
            states.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.stream == state.stream)
            });
            states.push(state.clone());
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ProviderPollState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        Box::pin(async move {
            Ok(states
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
        let states = self.states.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(states
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
        let delete_error = self.delete_error.clone();
        let delete_error_race_id = self.delete_error_race_id.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            if let Some(error) = delete_error {
                if delete_error_race_id
                    .as_ref()
                    .is_none_or(|target| target == &race_id)
                {
                    return Err(error);
                }
            }
            races
                .lock()
                .unwrap()
                .retain(|race| !(race.user_id == user_id && race.race_id == race_id));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingIntervalsService {
    pub(super) created_events: Arc<Mutex<Vec<CreateEvent>>>,
    pub(super) create_event_user_ids: Arc<Mutex<Vec<String>>>,
    pub(super) updated_events: Arc<Mutex<Vec<(i64, UpdateEvent)>>>,
    pub(super) update_event_user_ids: Arc<Mutex<Vec<String>>>,
    pub(super) deleted_event_ids: Arc<Mutex<Vec<i64>>>,
    listed_events: Arc<Mutex<Vec<Event>>>,
    pub(super) list_event_user_ids: Arc<Mutex<Vec<String>>>,
    fail_updates: bool,
}

#[derive(Clone, Default)]
pub(super) struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    pub(super) fn stored(&self) -> Vec<(String, String, String)> {
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

#[derive(Clone, Default)]
pub(super) struct FailingCalendarRefresh;

impl CalendarEntryViewRefreshPort for FailingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        Box::pin(async {
            Err(CalendarEntryViewError::Repository(
                "refresh boom".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct ClearingCalendarRefresh {
    views: Arc<Mutex<Vec<CalendarEntryView>>>,
}

impl ClearingCalendarRefresh {
    pub(super) fn with_views(views: Vec<CalendarEntryView>) -> Self {
        Self {
            views: Arc::new(Mutex::new(views)),
        }
    }

    pub(super) fn race_entry_ids(&self) -> Vec<String> {
        self.views
            .lock()
            .unwrap()
            .iter()
            .filter(|entry| entry.entry_kind == CalendarEntryKind::Race)
            .map(|entry| entry.entry_id.clone())
            .collect()
    }
}

impl CalendarEntryViewRefreshPort for ClearingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let views = self.views.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let mut stored = views.lock().unwrap();
            stored.retain(|entry| {
                !(entry.user_id == user_id
                    && entry.date >= oldest
                    && entry.date <= newest
                    && entry.entry_kind == CalendarEntryKind::Race)
            });
            Ok(stored.clone())
        })
    }
}

impl RecordingIntervalsService {
    pub(super) fn with_failed_updates() -> Self {
        Self {
            fail_updates: true,
            ..Self::default()
        }
    }

    pub(super) fn with_listed_events(events: Vec<Event>) -> Self {
        Self {
            listed_events: Arc::new(Mutex::new(events)),
            ..Self::default()
        }
    }
}

impl IntervalsUseCases for RecordingIntervalsService {
    fn list_events(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> IntervalsBoxFuture<Result<Vec<Event>, IntervalsError>> {
        let listed_events = self.listed_events.clone();
        let list_event_user_ids = self.list_event_user_ids.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            list_event_user_ids.lock().unwrap().push(user_id);
            Ok(listed_events.lock().unwrap().clone())
        })
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
        user_id: &str,
        event: CreateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let created_events = self.created_events.clone();
        let create_event_user_ids = self.create_event_user_ids.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            create_event_user_ids.lock().unwrap().push(user_id);
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
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let updated_events = self.updated_events.clone();
        let update_event_user_ids = self.update_event_user_ids.clone();
        let fail_updates = self.fail_updates;
        let user_id = user_id.to_string();
        Box::pin(async move {
            if fail_updates {
                return Err(IntervalsError::ConnectionError("boom".to_string()));
            }
            update_event_user_ids.lock().unwrap().push(user_id);
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
