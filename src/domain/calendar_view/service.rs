use crate::domain::{
    calendar::{NoopPlannedWorkoutSyncRepository, PlannedWorkoutSyncRepository},
    completed_workouts::CompletedWorkout,
    external_sync::ExternalSyncState,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
        NoopExternalSyncStateRepository,
    },
    intervals::DateRange,
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
pub struct CalendarEntryViewService<Repository, PlannedSyncs, SyncStates>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
    PlannedSyncs: PlannedWorkoutSyncRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
{
    repository: Repository,
    planned_workout_syncs: PlannedSyncs,
    sync_states: SyncStates,
}

impl<Repository>
    CalendarEntryViewService<
        Repository,
        NoopPlannedWorkoutSyncRepository,
        NoopExternalSyncStateRepository,
    >
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
{
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            planned_workout_syncs: NoopPlannedWorkoutSyncRepository,
            sync_states: NoopExternalSyncStateRepository,
        }
    }
}

impl<Repository, PlannedSyncs, SyncStates>
    CalendarEntryViewService<Repository, PlannedSyncs, SyncStates>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
    PlannedSyncs: PlannedWorkoutSyncRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
{
    pub fn with_sync_sources<NewPlannedSyncs, NewSyncStates>(
        self,
        planned_workout_syncs: NewPlannedSyncs,
        sync_states: NewSyncStates,
    ) -> CalendarEntryViewService<Repository, NewPlannedSyncs, NewSyncStates>
    where
        NewPlannedSyncs: PlannedWorkoutSyncRepository + Clone + 'static,
        NewSyncStates: ExternalSyncStateRepository + Clone + 'static,
    {
        CalendarEntryViewService {
            repository: self.repository,
            planned_workout_syncs,
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
        sync_state: Option<&ExternalSyncState>,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_planned_workout_entry(workout, sync_state);
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
        let planned_workout_syncs = self.planned_workout_syncs.clone();
        let sync_states = self.sync_states.clone();
        let user_id = user_id.to_string();
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
            let planned_syncs = planned_workout_syncs
                .list_by_user_id_and_range(
                    &user_id,
                    &DateRange {
                        oldest: CALENDAR_REBUILD_RANGE_START.to_string(),
                        newest: CALENDAR_REBUILD_RANGE_END.to_string(),
                    },
                )
                .await
                .map_err(map_planned_sync_error)?;
            let planned_syncs_by_id = planned_syncs
                .into_iter()
                .map(|record| (format!("{}:{}", record.operation_key, record.date), record))
                .collect::<std::collections::HashMap<_, _>>();
            let planned_sync_states_by_entity = sync_states
                .find_by_provider_and_canonical_entities(
                    &user_id,
                    ExternalProvider::Intervals,
                    &planned_entities,
                )
                .await
                .map_err(map_sync_error)?
                .into_iter()
                .map(|state| (state.canonical_entity.clone(), state))
                .collect::<std::collections::HashMap<_, _>>();
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
                        if let Some(sync_state) = planned_sync_states_by_entity.get(&planned_entity)
                        {
                            entry.sync = map_external_sync_state(Some(sync_state));
                            continue;
                        }
                        if let Some(record) = planned_syncs_by_id.get(planned_workout_id) {
                            entry.sync =
                                Some(map_planned_sync_record_to_calendar_entry_sync(record));
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

fn map_planned_sync_error(error: crate::domain::calendar::CalendarError) -> CalendarEntryViewError {
    match error {
        crate::domain::calendar::CalendarError::Validation(message)
        | crate::domain::calendar::CalendarError::Internal(message)
        | crate::domain::calendar::CalendarError::Unavailable(message) => {
            CalendarEntryViewError::Repository(message)
        }
        crate::domain::calendar::CalendarError::Unauthenticated => {
            CalendarEntryViewError::Repository(
                "planned workout sync refresh unauthenticated".to_string(),
            )
        }
        crate::domain::calendar::CalendarError::CredentialsNotConfigured => {
            CalendarEntryViewError::Repository(
                "planned workout sync credentials not configured".to_string(),
            )
        }
        crate::domain::calendar::CalendarError::NotFound => {
            CalendarEntryViewError::Repository("planned workout sync record not found".to_string())
        }
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

fn map_planned_sync_record_to_calendar_entry_sync(
    record: &crate::domain::calendar::PlannedWorkoutSyncRecord,
) -> super::CalendarEntrySync {
    super::CalendarEntrySync {
        linked_intervals_event_id: record.intervals_event_id,
        sync_status: Some(record.status.as_str().to_string()),
    }
}

fn map_external_sync_state(
    sync_state: Option<&ExternalSyncState>,
) -> Option<super::CalendarEntrySync> {
    sync_state.map(|state| super::CalendarEntrySync {
        linked_intervals_event_id: state
            .external_id
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok()),
        sync_status: Some(state.sync_status.as_str().to_string()),
    })
}
