use crate::domain::{
    calendar::{PlannedWorkoutSyncRecord, PlannedWorkoutSyncRepository},
    completed_workouts::CompletedWorkoutRepository,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
    },
    planned_workouts::PlannedWorkoutRepository,
    races::RaceRepository,
    special_days::SpecialDayRepository,
};

use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, BoxFuture, CalendarEntrySync, CalendarEntryView,
    CalendarEntryViewError, CalendarEntryViewRepository,
};

pub trait CalendarEntryViewRefreshPort: Clone + Send + Sync + 'static {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>>;
}

#[derive(Clone, Default)]
pub struct NoopCalendarEntryViewRefresh;

impl CalendarEntryViewRefreshPort for NoopCalendarEntryViewRefresh {
    fn refresh_range_for_user(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone)]
pub struct CalendarEntryViewRefreshService<
    Views,
    Planned,
    PlannedSyncs,
    Completed,
    Races,
    SpecialDays,
    SyncStates,
> where
    Views: CalendarEntryViewRepository + Clone,
    Planned: PlannedWorkoutRepository + Clone,
    PlannedSyncs: PlannedWorkoutSyncRepository + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    views: Views,
    planned_workouts: Planned,
    planned_workout_syncs: PlannedSyncs,
    completed_workouts: Completed,
    races: Races,
    special_days: SpecialDays,
    sync_states: SyncStates,
}

impl<Views, Planned, PlannedSyncs, Completed, Races, SpecialDays, SyncStates>
    CalendarEntryViewRefreshService<
        Views,
        Planned,
        PlannedSyncs,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
    >
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: PlannedWorkoutRepository + Clone,
    PlannedSyncs: PlannedWorkoutSyncRepository + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    pub fn new(
        views: Views,
        planned_workouts: Planned,
        planned_workout_syncs: PlannedSyncs,
        completed_workouts: Completed,
        races: Races,
        special_days: SpecialDays,
        sync_states: SyncStates,
    ) -> Self {
        Self {
            views,
            planned_workouts,
            planned_workout_syncs,
            completed_workouts,
            races,
            special_days,
            sync_states,
        }
    }
}

impl<Views, Planned, PlannedSyncs, Completed, Races, SpecialDays, SyncStates>
    CalendarEntryViewRefreshPort
    for CalendarEntryViewRefreshService<
        Views,
        Planned,
        PlannedSyncs,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
    >
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: PlannedWorkoutRepository + Clone,
    PlannedSyncs: PlannedWorkoutSyncRepository + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let views = self.views.clone();
        let planned_workouts = self.planned_workouts.clone();
        let planned_workout_syncs = self.planned_workout_syncs.clone();
        let completed_workouts = self.completed_workouts.clone();
        let races = self.races.clone();
        let special_days = self.special_days.clone();
        let sync_states = self.sync_states.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let planned = planned_workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_planned_error)?;
            let completed = completed_workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_completed_error)?;
            let planned_syncs = planned_workout_syncs
                .list_by_user_id_and_range(
                    &user_id,
                    &crate::domain::intervals::DateRange {
                        oldest: oldest.clone(),
                        newest: newest.clone(),
                    },
                )
                .await
                .map_err(map_planned_sync_error)?;
            let races = races
                .list_by_user_id_and_range(
                    &user_id,
                    &crate::domain::intervals::DateRange {
                        oldest: oldest.clone(),
                        newest: newest.clone(),
                    },
                )
                .await
                .map_err(map_race_error)?;
            let special_days = special_days
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_special_day_error)?;

            let planned_syncs_by_id = planned_syncs
                .into_iter()
                .map(|record| (format!("{}:{}", record.operation_key, record.date), record))
                .collect::<std::collections::HashMap<_, _>>();

            let mut projected = Vec::with_capacity(planned.len());
            for workout in &planned {
                let planned_entity = CanonicalEntityRef::new(
                    CanonicalEntityKind::PlannedWorkout,
                    workout.planned_workout_id.clone(),
                );
                let sync_state = sync_states
                    .find_by_provider_and_canonical_entity(
                        &user_id,
                        ExternalProvider::Intervals,
                        &planned_entity,
                    )
                    .await
                    .map_err(map_sync_error)?;
                let mut entry = project_planned_workout_entry(workout, sync_state.as_ref());
                if entry.sync.is_none() {
                    entry.sync = planned_syncs_by_id
                        .get(&workout.planned_workout_id)
                        .map(map_planned_sync_record_to_calendar_entry_sync);
                }
                projected.push(entry);
            }
            projected.extend(completed.iter().map(project_completed_workout_entry));
            for race in &races {
                let sync_state = sync_states
                    .find_by_provider_and_canonical_entity(
                        &user_id,
                        ExternalProvider::Intervals,
                        &CanonicalEntityRef::new(CanonicalEntityKind::Race, race.race_id.clone()),
                    )
                    .await
                    .map_err(map_sync_error)?;
                projected.push(project_race_entry(race, sync_state.as_ref()));
            }
            projected.extend(special_days.iter().map(project_special_day_entry));
            projected.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.entry_kind.as_str().cmp(right.entry_kind.as_str()))
                    .then_with(|| left.entry_id.cmp(&right.entry_id))
            });

            views
                .replace_range_for_user(&user_id, &oldest, &newest, projected)
                .await
        })
    }
}

fn map_planned_error(
    error: crate::domain::planned_workouts::PlannedWorkoutError,
) -> CalendarEntryViewError {
    match error {
        crate::domain::planned_workouts::PlannedWorkoutError::Repository(message) => {
            CalendarEntryViewError::Repository(message)
        }
    }
}

fn map_completed_error(
    error: crate::domain::completed_workouts::CompletedWorkoutError,
) -> CalendarEntryViewError {
    match error {
        crate::domain::completed_workouts::CompletedWorkoutError::Repository(message) => {
            CalendarEntryViewError::Repository(message)
        }
    }
}

fn map_race_error(error: crate::domain::races::RaceError) -> CalendarEntryViewError {
    match error {
        crate::domain::races::RaceError::Validation(message)
        | crate::domain::races::RaceError::Unavailable(message)
        | crate::domain::races::RaceError::Internal(message) => {
            CalendarEntryViewError::Repository(message)
        }
        crate::domain::races::RaceError::Unauthenticated => {
            CalendarEntryViewError::Repository("race refresh unauthenticated".to_string())
        }
        crate::domain::races::RaceError::NotFound => {
            CalendarEntryViewError::Repository("race refresh not found".to_string())
        }
    }
}

fn map_special_day_error(
    error: crate::domain::special_days::SpecialDayError,
) -> CalendarEntryViewError {
    match error {
        crate::domain::special_days::SpecialDayError::Repository(message) => {
            CalendarEntryViewError::Repository(message)
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

fn map_planned_sync_error(error: crate::domain::calendar::CalendarError) -> CalendarEntryViewError {
    match error {
        crate::domain::calendar::CalendarError::NotFound => {
            CalendarEntryViewError::Repository("planned workout sync not found".to_string())
        }
        crate::domain::calendar::CalendarError::Unauthenticated => {
            CalendarEntryViewError::Repository("planned workout sync unauthenticated".to_string())
        }
        crate::domain::calendar::CalendarError::CredentialsNotConfigured => {
            CalendarEntryViewError::Repository(
                "planned workout sync credentials not configured".to_string(),
            )
        }
        crate::domain::calendar::CalendarError::Validation(message)
        | crate::domain::calendar::CalendarError::Unavailable(message)
        | crate::domain::calendar::CalendarError::Internal(message) => {
            CalendarEntryViewError::Repository(message)
        }
    }
}

fn map_planned_sync_record_to_calendar_entry_sync(
    record: &PlannedWorkoutSyncRecord,
) -> CalendarEntrySync {
    CalendarEntrySync {
        linked_intervals_event_id: record.intervals_event_id,
        sync_status: Some(record.status.as_str().to_string()),
    }
}
