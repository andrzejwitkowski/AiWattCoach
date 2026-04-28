use crate::domain::{
    completed_workouts::CompletedWorkout,
    external_sync::ExternalSyncState,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
        NoopExternalSyncStateRepository,
    },
    planned_workouts::PlannedWorkout,
    races::Race,
    special_days::SpecialDay,
};

use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, rebuild_calendar_entries, BoxFuture, CalendarEntryKind,
    CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRepository,
};

const CALENDAR_REBUILD_RANGE_START: &str = "0000-01-01";
const CALENDAR_REBUILD_RANGE_END: &str = "9999-12-31";

#[derive(Clone)]
pub struct CalendarEntryViewService<Repository, SyncStates>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
{
    repository: Repository,
    sync_states: SyncStates,
}

impl<Repository> CalendarEntryViewService<Repository, NoopExternalSyncStateRepository>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
{
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            sync_states: NoopExternalSyncStateRepository,
        }
    }
}

impl<Repository, SyncStates> CalendarEntryViewService<Repository, SyncStates>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
{
    pub fn with_sync_states<NewSyncStates>(
        self,
        sync_states: NewSyncStates,
    ) -> CalendarEntryViewService<Repository, NewSyncStates>
    where
        NewSyncStates: ExternalSyncStateRepository + Clone + 'static,
    {
        CalendarEntryViewService {
            repository: self.repository,
            sync_states,
        }
    }

    pub fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
        })
    }

    pub fn upsert_planned_workout(
        &self,
        workout: &PlannedWorkout,
        sync_states: &[ExternalSyncState],
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_planned_workout_entry(workout, sync_states);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_completed_workout(
        &self,
        workout: &CompletedWorkout,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_completed_workout_entry(workout);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_race(
        &self,
        race: &Race,
        sync_state: Option<&ExternalSyncState>,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_race_entry(race, sync_state);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_special_day(
        &self,
        special_day: &SpecialDay,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_special_day_entry(special_day);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn rebuild_for_user(
        &self,
        user_id: &str,
        planned_workouts: &[PlannedWorkout],
        completed_workouts: &[CompletedWorkout],
        races: &[Race],
        special_days: &[SpecialDay],
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let sync_states = self.sync_states.clone();
        let user_id = user_id.to_string();
        let planned_workouts_by_id = planned_workouts
            .iter()
            .cloned()
            .map(|workout| (workout.planned_workout_id.clone(), workout))
            .collect::<std::collections::HashMap<_, _>>();
        let race_ids = races
            .iter()
            .map(|race| race.race_id.clone())
            .collect::<Vec<_>>();
        let planned_entities = planned_workouts
            .iter()
            .map(|workout| {
                CanonicalEntityRef::new(
                    CanonicalEntityKind::PlannedWorkout,
                    workout.planned_workout_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let mut entries =
            rebuild_calendar_entries(planned_workouts, completed_workouts, races, special_days);
        Box::pin(async move {
            let all_sync_states = sync_states
                .find_by_canonical_entities(&user_id, &planned_entities)
                .await
                .map_err(map_sync_error)?;
            let planned_sync_states_by_entity = all_sync_states.into_iter().fold(
                std::collections::HashMap::<CanonicalEntityRef, Vec<ExternalSyncState>>::new(),
                |mut acc, state| {
                    acc.entry(state.canonical_entity.clone())
                        .or_default()
                        .push(state);
                    acc
                },
            );
            let mut race_syncs_by_id = std::collections::HashMap::new();
            for race_id in race_ids {
                let sync_state = sync_states
                    .find_by_provider_and_canonical_entity(
                        &user_id,
                        ExternalProvider::Intervals,
                        &CanonicalEntityRef::new(CanonicalEntityKind::Race, race_id.clone()),
                    )
                    .await
                    .map_err(map_sync_error)?;
                if let Some(sync_state) = sync_state {
                    race_syncs_by_id.insert(race_id, sync_state);
                }
            }
            let existing_entries = repository
                .list_by_user_id_and_date_range(
                    &user_id,
                    CALENDAR_REBUILD_RANGE_START,
                    CALENDAR_REBUILD_RANGE_END,
                )
                .await?;
            let sync_by_entry_id = existing_entries
                .into_iter()
                .filter_map(|entry| entry.sync.map(|sync| (entry.entry_id, sync)))
                .collect::<std::collections::HashMap<_, _>>();
            for entry in &mut entries {
                if entry.entry_kind == CalendarEntryKind::PlannedWorkout {
                    if let Some(planned_workout_id) = &entry.planned_workout_id {
                        let planned_entity = CanonicalEntityRef::new(
                            CanonicalEntityKind::PlannedWorkout,
                            planned_workout_id.clone(),
                        );
                        if let Some(sync_states) =
                            planned_sync_states_by_entity.get(&planned_entity)
                        {
                            entry.sync = planned_workouts_by_id
                                .get(planned_workout_id)
                                .and_then(|workout| {
                                    project_planned_workout_entry(workout, sync_states).sync
                                })
                                .or_else(|| map_external_sync_states(sync_states));
                            continue;
                        }
                    }
                }
                if let Some(race_id) = &entry.race_id {
                    if let Some(sync_state) = race_syncs_by_id.get(race_id) {
                        entry.sync = map_external_sync_state(Some(sync_state));
                        continue;
                    }
                }
                if let Some(sync) = sync_by_entry_id.get(&entry.entry_id) {
                    entry.sync = Some(sync.clone());
                }
            }
            repository.replace_all_for_user(&user_id, entries).await
        })
    }
}

fn map_sync_error(
    error: crate::domain::external_sync::ExternalSyncRepositoryError,
) -> CalendarEntryViewError {
    match error {
        crate::domain::external_sync::ExternalSyncRepositoryError::Storage(message)
        | crate::domain::external_sync::ExternalSyncRepositoryError::CorruptData(message) => {
            CalendarEntryViewError::Repository(message)
        }
    }
}

fn map_external_sync_state(
    sync_state: Option<&ExternalSyncState>,
) -> Option<super::CalendarEntrySync> {
    sync_state.map(|state| super::CalendarEntrySync {
        linked_intervals_event_id: linked_intervals_event_id(state),
        sync_status: Some(state.sync_status.as_str().to_string()),
    })
}

fn map_external_sync_states(sync_states: &[ExternalSyncState]) -> Option<super::CalendarEntrySync> {
    if sync_states.is_empty() {
        return None;
    }

    let linked_intervals_event_id = sync_states.iter().find_map(linked_intervals_event_id);
    let sync_status = if sync_states
        .iter()
        .any(|state| state.sync_status.as_str() == "synced")
    {
        Some("synced".to_string())
    } else if sync_states
        .iter()
        .any(|state| state.sync_status.as_str() == "pending")
    {
        Some("pending".to_string())
    } else if sync_states
        .iter()
        .any(|state| state.sync_status.as_str() == "failed")
    {
        Some("failed".to_string())
    } else {
        None
    };

    Some(super::CalendarEntrySync {
        linked_intervals_event_id,
        sync_status,
    })
}

fn linked_intervals_event_id(state: &ExternalSyncState) -> Option<i64> {
    if state.provider != ExternalProvider::Intervals {
        return None;
    }

    state.external_id.as_deref().map(|value| {
        value.parse::<i64>().unwrap_or_else(|error| {
            panic!("intervals sync state external_id must parse as i64, got '{value}': {error}")
        })
    })
}
