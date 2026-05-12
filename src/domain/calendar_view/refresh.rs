use chrono::NaiveDate;

use crate::domain::{
    calendar_view::{
        select_visible_planned_workout_candidates_with_sync_states, CalendarPlannedWorkoutSource,
    },
    completed_workouts::{CompletedWorkout, CompletedWorkoutRepository},
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
    },
    planned_completed_links::{
        PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
        PlannedCompletedWorkoutLinkRepository,
    },
    planned_workouts::PlannedWorkout,
    races::RaceRepository,
    special_days::SpecialDayRepository,
};

use super::{
    merge_workout_entries, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, BoxFuture, CalendarEntryView, CalendarEntryViewError,
    CalendarEntryViewRepository,
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

#[derive(Clone, Default)]
pub struct NoopPlannedCompletedWorkoutLinkRepository;

impl PlannedCompletedWorkoutLinkRepository for NoopPlannedCompletedWorkoutLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        _user_id: &str,
        _planned_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Option<crate::domain::planned_completed_links::PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Option<crate::domain::planned_completed_links::PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_planned_workout_ids(
        &self,
        _user_id: &str,
        _planned_workout_ids: &[String],
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Vec<crate::domain::planned_completed_links::PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        link: crate::domain::planned_completed_links::PlannedCompletedWorkoutLink,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLink,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        Box::pin(async move { Ok(link) })
    }

    fn delete_by_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<(), crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError>,
    > {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone)]
pub struct CalendarEntryViewRefreshService<
    Views,
    Planned,
    Completed,
    Races,
    SpecialDays,
    SyncStates,
    CleanupPlanned = Planned,
    PlannedCompletedLinks = NoopPlannedCompletedWorkoutLinkRepository,
> where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    CleanupPlanned: CalendarPlannedWorkoutSource + Clone,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone,
{
    views: Views,
    planned_workouts: Planned,
    cleanup_planned_workouts: CleanupPlanned,
    completed_workouts: Completed,
    races: Races,
    special_days: SpecialDays,
    sync_states: SyncStates,
    planned_completed_links: PlannedCompletedLinks,
}

impl<Views, Planned, Completed, Races, SpecialDays, SyncStates>
    CalendarEntryViewRefreshService<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        Planned,
        NoopPlannedCompletedWorkoutLinkRepository,
    >
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    pub fn new(
        views: Views,
        planned_workouts: Planned,
        completed_workouts: Completed,
        races: Races,
        special_days: SpecialDays,
        sync_states: SyncStates,
    ) -> Self {
        Self {
            views,
            cleanup_planned_workouts: planned_workouts.clone(),
            planned_workouts,
            completed_workouts,
            races,
            special_days,
            sync_states,
            planned_completed_links: NoopPlannedCompletedWorkoutLinkRepository,
        }
    }
}

impl<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        CleanupPlanned,
        PlannedCompletedLinks,
    >
    CalendarEntryViewRefreshService<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        CleanupPlanned,
        PlannedCompletedLinks,
    >
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    CleanupPlanned: CalendarPlannedWorkoutSource + Clone,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone,
{
    pub fn with_cleanup_planned_workouts<NewCleanupPlanned>(
        self,
        cleanup_planned_workouts: NewCleanupPlanned,
    ) -> CalendarEntryViewRefreshService<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        NewCleanupPlanned,
        PlannedCompletedLinks,
    >
    where
        NewCleanupPlanned: CalendarPlannedWorkoutSource + Clone,
    {
        CalendarEntryViewRefreshService {
            views: self.views,
            planned_workouts: self.planned_workouts,
            cleanup_planned_workouts,
            completed_workouts: self.completed_workouts,
            races: self.races,
            special_days: self.special_days,
            sync_states: self.sync_states,
            planned_completed_links: self.planned_completed_links,
        }
    }

    pub fn with_planned_completed_links<NewPlannedCompletedLinks>(
        self,
        planned_completed_links: NewPlannedCompletedLinks,
    ) -> CalendarEntryViewRefreshService<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        CleanupPlanned,
        NewPlannedCompletedLinks,
    >
    where
        NewPlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone,
    {
        CalendarEntryViewRefreshService {
            views: self.views,
            planned_workouts: self.planned_workouts,
            cleanup_planned_workouts: self.cleanup_planned_workouts,
            completed_workouts: self.completed_workouts,
            races: self.races,
            special_days: self.special_days,
            sync_states: self.sync_states,
            planned_completed_links,
        }
    }
}

impl<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        CleanupPlanned,
        PlannedCompletedLinks,
    > CalendarEntryViewRefreshPort
    for CalendarEntryViewRefreshService<
        Views,
        Planned,
        Completed,
        Races,
        SpecialDays,
        SyncStates,
        CleanupPlanned,
        PlannedCompletedLinks,
    >
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
    CleanupPlanned: CalendarPlannedWorkoutSource + Clone,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone,
{
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let views = self.views.clone();
        let planned_workouts = self.planned_workouts.clone();
        let cleanup_planned_workouts = self.cleanup_planned_workouts.clone();
        let completed_workouts = self.completed_workouts.clone();
        let races = self.races.clone();
        let special_days = self.special_days.clone();
        let sync_states = self.sync_states.clone();
        let planned_completed_links = self.planned_completed_links.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let all_planned_ids = cleanup_planned_workouts
                .list_visible_planned_workout_ids_by_user_id(&user_id)
                .await
                .map_err(map_planned_error)?
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let planned_candidates = planned_workouts
                .list_candidates_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_planned_error)?;
            let candidate_entities = planned_candidates
                .iter()
                .map(|candidate| {
                    CanonicalEntityRef::new(
                        CanonicalEntityKind::PlannedWorkout,
                        candidate.workout.planned_workout_id.clone(),
                    )
                })
                .collect::<Vec<_>>();
            let planned_sync_states_by_entity = sync_states
                .find_by_canonical_entities(&user_id, &candidate_entities)
                .await
                .map_err(map_sync_error)?
                .into_iter()
                .fold(
                    std::collections::HashMap::<
                        CanonicalEntityRef,
                        Vec<crate::domain::external_sync::ExternalSyncState>,
                    >::new(),
                    |mut acc, state| {
                        acc.entry(state.canonical_entity.clone())
                            .or_default()
                            .push(state);
                        acc
                    },
                );
            let planned_sync_states_by_id = planned_candidates
                .iter()
                .map(|candidate| {
                    let entity = CanonicalEntityRef::new(
                        CanonicalEntityKind::PlannedWorkout,
                        candidate.workout.planned_workout_id.clone(),
                    );
                    (
                        candidate.workout.planned_workout_id.clone(),
                        planned_sync_states_by_entity
                            .get(&entity)
                            .cloned()
                            .unwrap_or_default(),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>();
            let planned = select_visible_planned_workout_candidates_with_sync_states(
                planned_candidates,
                &planned_sync_states_by_id,
            )
            .into_iter()
            .map(|candidate| candidate.workout)
            .collect::<Vec<_>>();
            let completed = completed_workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_completed_error)?;
            for workout in &completed {
                let existing_link = planned_completed_links
                    .find_by_completed_workout_id(&user_id, &workout.completed_workout_id)
                    .await
                    .map_err(map_planned_completed_link_error)?;

                if let Some(link) = existing_link.as_ref().filter(|link| {
                    link.match_source != PlannedCompletedWorkoutLinkMatchSource::Heuristic
                }) {
                    if workout.planned_workout_id.as_deref()
                        != Some(link.planned_workout_id.as_str())
                    {
                        let mut updated = workout.clone();
                        updated.planned_workout_id = Some(link.planned_workout_id.clone());
                        completed_workouts
                            .upsert(updated)
                            .await
                            .map_err(map_completed_error)?;
                    }
                    continue;
                }

                if let Some(relinked_planned_workout_id) =
                    resolve_unique_same_day_planned_workout_id(&planned, workout)
                {
                    if workout.planned_workout_id.as_deref()
                        != Some(relinked_planned_workout_id.as_str())
                        || existing_link
                            .as_ref()
                            .map(|link| link.planned_workout_id.as_str())
                            != Some(relinked_planned_workout_id.as_str())
                    {
                        let mut updated = workout.clone();
                        updated.planned_workout_id = Some(relinked_planned_workout_id.clone());
                        completed_workouts
                            .upsert(updated.clone())
                            .await
                            .map_err(map_completed_error)?;
                        if let Some(matched_at_epoch_seconds) =
                            heuristic_link_timestamp(&updated.start_date_local)
                        {
                            planned_completed_links
                                .upsert(PlannedCompletedWorkoutLink::new(
                                    user_id.clone(),
                                    relinked_planned_workout_id,
                                    updated.completed_workout_id.clone(),
                                    PlannedCompletedWorkoutLinkMatchSource::Heuristic,
                                    matched_at_epoch_seconds,
                                ))
                                .await
                                .map_err(map_planned_completed_link_error)?;
                        }
                    }
                    continue;
                }

                let Some(planned_workout_id) = workout.planned_workout_id.as_deref() else {
                    continue;
                };
                if all_planned_ids.contains(planned_workout_id) {
                    continue;
                }
                let link = existing_link;
                if matches!(
                    link.as_ref().map(|link| &link.match_source),
                    Some(source) if source != &PlannedCompletedWorkoutLinkMatchSource::Heuristic
                ) {
                    continue;
                }

                if link.is_some() {
                    planned_completed_links
                        .delete_by_completed_workout_id(&user_id, &workout.completed_workout_id)
                        .await
                        .map_err(map_planned_completed_link_error)?;
                }
                let mut updated = workout.clone();
                updated.planned_workout_id = None;
                completed_workouts
                    .upsert(updated)
                    .await
                    .map_err(map_completed_error)?;
            }
            let completed = completed_workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
                .map_err(map_completed_error)?;
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

            let planned_entities = planned
                .iter()
                .map(|workout| {
                    CanonicalEntityRef::new(
                        CanonicalEntityKind::PlannedWorkout,
                        workout.planned_workout_id.clone(),
                    )
                })
                .collect::<Vec<_>>();
            let planned_sync_states_by_entity = sync_states
                .find_by_canonical_entities(&user_id, &planned_entities)
                .await
                .map_err(map_sync_error)?
                .into_iter()
                .fold(
                    std::collections::HashMap::<
                        CanonicalEntityRef,
                        Vec<crate::domain::external_sync::ExternalSyncState>,
                    >::new(),
                    |mut acc, state| {
                        acc.entry(state.canonical_entity.clone())
                            .or_default()
                            .push(state);
                        acc
                    },
                );

            let mut projected_planned = Vec::with_capacity(planned.len());
            for workout in &planned {
                let planned_entity = CanonicalEntityRef::new(
                    CanonicalEntityKind::PlannedWorkout,
                    workout.planned_workout_id.clone(),
                );
                let sync_states = planned_sync_states_by_entity
                    .get(&planned_entity)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let entry = project_planned_workout_entry(workout, sync_states);
                projected_planned.push(entry);
            }
            let mut projected = merge_workout_entries(projected_planned, &completed);
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

fn heuristic_link_timestamp(start_date_local: &str) -> Option<i64> {
    let date = start_date_local
        .split_once('T')
        .map(|(date, _)| date)
        .unwrap_or(start_date_local);

    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(12, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
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
        crate::domain::special_days::SpecialDayError::Validation(message)
        | crate::domain::special_days::SpecialDayError::Repository(message) => {
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

fn map_planned_completed_link_error(
    error: crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
) -> CalendarEntryViewError {
    match error {
        crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError::Repository(
            message,
        ) => CalendarEntryViewError::Repository(message),
    }
}

fn resolve_unique_same_day_planned_workout_id(
    planned_workouts: &[PlannedWorkout],
    completed_workout: &CompletedWorkout,
) -> Option<String> {
    let completed_name = normalize_workout_name(completed_workout.name.as_deref())?;
    let completed_date = completed_workout
        .start_date_local
        .get(..10)
        .unwrap_or(completed_workout.start_date_local.as_str());

    let mut matches = planned_workouts
        .iter()
        .filter(|planned_workout| planned_workout.date == completed_date)
        .filter(|planned_workout| {
            normalize_workout_name(planned_workout_match_name(planned_workout).as_deref())
                .as_deref()
                == Some(completed_name.as_str())
        })
        .map(|planned_workout| planned_workout.planned_workout_id.clone())
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();

    match matches.as_slice() {
        [planned_workout_id] => Some(planned_workout_id.clone()),
        _ => None,
    }
}

fn planned_workout_match_name(workout: &PlannedWorkout) -> Option<String> {
    workout.name.clone().or_else(|| {
        workout.workout.lines.iter().find_map(|line| match line {
            crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
                Some(text.text.clone())
            }
            _ => None,
        })
    })
}

fn normalize_workout_name(value: Option<&str>) -> Option<String> {
    let normalized = value?.trim();
    if normalized.is_empty() {
        return None;
    }

    Some(
        normalized
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}
