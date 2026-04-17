mod completed_workout_dedup;
mod date_keys;
mod import_outcome;
#[cfg(test)]
mod tests;

use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    completed_workouts::{CompletedWorkout, CompletedWorkoutError, CompletedWorkoutRepository},
    identity::Clock,
    planned_completed_links::{
        PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
        PlannedCompletedWorkoutLinkRepository,
    },
    planned_workout_tokens::{extract_planned_workout_marker, PlannedWorkoutTokenRepository},
    planned_workouts::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository},
    races::{Race, RaceError, RaceRepository},
    special_days::{SpecialDay, SpecialDayError, SpecialDayRepository},
};

use self::completed_workout_dedup::{completed_workout_dedup_key, merge_completed_workout};
use self::{
    date_keys::date_key,
    import_outcome::{finalize_import, map_repository_error, sync_metadata_input},
};
use super::{
    BoxFuture, CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
    ExternalObservationParams, ExternalObservationRepository, ExternalProvider, ExternalSyncState,
    ExternalSyncStateRepository,
};

#[derive(Clone)]
struct ResolvedPlannedWorkoutLink {
    planned_workout_id: String,
    match_source: PlannedCompletedWorkoutLinkMatchSource,
}

struct ResolvedCompletedWorkoutTarget {
    workout: CompletedWorkout,
    refresh_dates: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExternalImportCommand {
    UpsertPlannedWorkout(ExternalPlannedWorkoutImport),
    UpsertCompletedWorkout(Box<ExternalCompletedWorkoutImport>),
    UpsertRace(ExternalRaceImport),
    UpsertSpecialDay(ExternalSpecialDayImport),
}

struct SyncMetadataInput {
    provider: ExternalProvider,
    external_object_kind: ExternalObjectKind,
    external_id: String,
    canonical_entity: CanonicalEntityRef,
    normalized_payload_hash: String,
    dedup_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalPlannedWorkoutImport {
    pub provider: ExternalProvider,
    pub external_id: String,
    pub normalized_payload_hash: String,
    pub workout: PlannedWorkout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalCompletedWorkoutImport {
    pub provider: ExternalProvider,
    pub external_id: String,
    pub normalized_payload_hash: String,
    pub marker_sources: Vec<String>,
    pub workout: CompletedWorkout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalRaceImport {
    pub provider: ExternalProvider,
    pub external_id: String,
    pub normalized_payload_hash: String,
    pub race: Race,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalSpecialDayImport {
    pub provider: ExternalProvider,
    pub external_id: String,
    pub normalized_payload_hash: String,
    pub special_day: SpecialDay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalImportOutcome {
    pub canonical_entity: CanonicalEntityRef,
    pub provider: ExternalProvider,
    pub external_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalImportError {
    PlannedWorkout(String),
    CompletedWorkout(String),
    Race(String),
    SpecialDay(String),
    Repository(String),
}

impl std::fmt::Display for ExternalImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlannedWorkout(message)
            | Self::CompletedWorkout(message)
            | Self::Race(message)
            | Self::SpecialDay(message)
            | Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ExternalImportError {}

pub trait ExternalImportUseCases: Clone + Send + Sync + 'static {
    fn import(
        &self,
        command: ExternalImportCommand,
    ) -> BoxFuture<Result<ExternalImportOutcome, ExternalImportError>>;
}

#[derive(Clone)]
pub struct ExternalImportService<
    PlannedWorkouts,
    CompletedWorkouts,
    Races,
    SpecialDays,
    PlannedWorkoutTokens,
    PlannedCompletedLinks,
    Observations,
    SyncStates,
    Time,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    PlannedWorkoutTokens: PlannedWorkoutTokenRepository + Clone + 'static,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    planned_workouts: PlannedWorkouts,
    completed_workouts: CompletedWorkouts,
    races: Races,
    special_days: SpecialDays,
    planned_workout_tokens: PlannedWorkoutTokens,
    planned_completed_links: PlannedCompletedLinks,
    observations: Observations,
    sync_states: SyncStates,
    clock: Time,
    refresh: Refresh,
}

impl<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
    >
    ExternalImportService<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
    >
where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    PlannedWorkoutTokens: PlannedWorkoutTokenRepository + Clone + 'static,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "external import service coordinates canonical roots, matching metadata, and refresh dependencies"
    )]
    pub fn new(
        planned_workouts: PlannedWorkouts,
        completed_workouts: CompletedWorkouts,
        races: Races,
        special_days: SpecialDays,
        planned_workout_tokens: PlannedWorkoutTokens,
        planned_completed_links: PlannedCompletedLinks,
        observations: Observations,
        sync_states: SyncStates,
        clock: Time,
    ) -> Self {
        Self {
            planned_workouts,
            completed_workouts,
            races,
            special_days,
            planned_workout_tokens,
            planned_completed_links,
            observations,
            sync_states,
            clock,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
        Refresh,
    >
    ExternalImportService<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
        Refresh,
    >
where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    PlannedWorkoutTokens: PlannedWorkoutTokenRepository + Clone + 'static,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> ExternalImportService<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
        NewRefresh,
    >
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone + 'static,
    {
        ExternalImportService {
            planned_workouts: self.planned_workouts,
            completed_workouts: self.completed_workouts,
            races: self.races,
            special_days: self.special_days,
            planned_workout_tokens: self.planned_workout_tokens,
            planned_completed_links: self.planned_completed_links,
            observations: self.observations,
            sync_states: self.sync_states,
            clock: self.clock,
            refresh,
        }
    }

    pub fn import(
        &self,
        command: ExternalImportCommand,
    ) -> BoxFuture<Result<ExternalImportOutcome, ExternalImportError>> {
        let service = self.clone();
        Box::pin(async move {
            match command {
                ExternalImportCommand::UpsertPlannedWorkout(command) => {
                    service.import_planned_workout(command).await
                }
                ExternalImportCommand::UpsertCompletedWorkout(command) => {
                    service.import_completed_workout(*command).await
                }
                ExternalImportCommand::UpsertRace(command) => service.import_race(command).await,
                ExternalImportCommand::UpsertSpecialDay(command) => {
                    service.import_special_day(command).await
                }
            }
        })
    }

    async fn import_planned_workout(
        &self,
        command: ExternalPlannedWorkoutImport,
    ) -> Result<ExternalImportOutcome, ExternalImportError> {
        let workout = self
            .planned_workouts
            .upsert(command.workout)
            .await
            .map_err(map_planned_workout_error)?;
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            workout.planned_workout_id.clone(),
        );
        let metadata = sync_metadata_input(
            command.provider.clone(),
            ExternalObjectKind::PlannedWorkout,
            command.external_id.clone(),
            canonical_entity.clone(),
            command.normalized_payload_hash,
            None,
        );

        finalize_import(
            &self.refresh,
            self.persist_sync_metadata(&workout.user_id, metadata),
            &workout.user_id,
            std::slice::from_ref(&workout.date),
            command.provider,
            command.external_id,
            canonical_entity,
        )
        .await
    }

    async fn import_completed_workout(
        &self,
        command: ExternalCompletedWorkoutImport,
    ) -> Result<ExternalImportOutcome, ExternalImportError> {
        let dedup_key = completed_workout_dedup_key(&command.workout);
        let resolved_link = self
            .resolve_planned_workout_id_for_completed_workout(
                &command.workout.user_id,
                &command.marker_sources,
                &command.workout,
            )
            .await?;
        let resolved_target = self
            .resolve_completed_workout_target(command.workout, dedup_key.as_deref())
            .await?;
        let mut workout = resolved_target.workout;
        let refresh_dates = resolved_target.refresh_dates;
        let existing_link = self
            .planned_completed_links
            .find_by_completed_workout_id(&workout.user_id, &workout.completed_workout_id)
            .await
            .map_err(map_planned_completed_link_error)?;
        if existing_link.is_none() {
            persist_legacy_planned_workout_link(
                &self.planned_completed_links,
                &workout,
                self.clock.now_epoch_seconds(),
            )
            .await?;
        }
        let existing_link = self
            .planned_completed_links
            .find_by_completed_workout_id(&workout.user_id, &workout.completed_workout_id)
            .await
            .map_err(map_planned_completed_link_error)?;
        let selected_link = choose_preferred_planned_workout_link(
            existing_link_candidate(existing_link.as_ref()),
            resolved_link,
        );
        workout.planned_workout_id = selected_link
            .as_ref()
            .map(|link| link.planned_workout_id.clone());
        let workout = self
            .completed_workouts
            .upsert(workout)
            .await
            .map_err(map_completed_workout_error)?;
        if let Some(link) = selected_link {
            self.planned_completed_links
                .upsert(PlannedCompletedWorkoutLink::new(
                    workout.user_id.clone(),
                    link.planned_workout_id,
                    workout.completed_workout_id.clone(),
                    link.match_source,
                    self.clock.now_epoch_seconds(),
                ))
                .await
                .map_err(map_planned_completed_link_error)?;
        }
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            workout.completed_workout_id.clone(),
        );
        let metadata = sync_metadata_input(
            command.provider.clone(),
            ExternalObjectKind::CompletedWorkout,
            command.external_id.clone(),
            canonical_entity.clone(),
            command.normalized_payload_hash,
            dedup_key,
        );

        finalize_import(
            &self.refresh,
            self.persist_sync_metadata(&workout.user_id, metadata),
            &workout.user_id,
            &refresh_dates,
            command.provider,
            command.external_id,
            canonical_entity,
        )
        .await
    }

    async fn import_race(
        &self,
        command: ExternalRaceImport,
    ) -> Result<ExternalImportOutcome, ExternalImportError> {
        let race = self
            .races
            .upsert(command.race)
            .await
            .map_err(map_race_error)?;
        let canonical_entity =
            CanonicalEntityRef::new(CanonicalEntityKind::Race, race.race_id.clone());
        let metadata = sync_metadata_input(
            command.provider.clone(),
            ExternalObjectKind::Race,
            command.external_id.clone(),
            canonical_entity.clone(),
            command.normalized_payload_hash,
            None,
        );

        finalize_import(
            &self.refresh,
            self.persist_sync_metadata(&race.user_id, metadata),
            &race.user_id,
            std::slice::from_ref(&race.date),
            command.provider,
            command.external_id,
            canonical_entity,
        )
        .await
    }

    async fn import_special_day(
        &self,
        command: ExternalSpecialDayImport,
    ) -> Result<ExternalImportOutcome, ExternalImportError> {
        let special_day = self
            .special_days
            .upsert(command.special_day)
            .await
            .map_err(map_special_day_error)?;
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::SpecialDay,
            special_day.special_day_id.clone(),
        );
        let metadata = sync_metadata_input(
            command.provider.clone(),
            ExternalObjectKind::SpecialDay,
            command.external_id.clone(),
            canonical_entity.clone(),
            command.normalized_payload_hash,
            None,
        );

        finalize_import(
            &self.refresh,
            self.persist_sync_metadata(&special_day.user_id, metadata),
            &special_day.user_id,
            std::slice::from_ref(&special_day.date),
            command.provider,
            command.external_id,
            canonical_entity,
        )
        .await
    }

    async fn persist_sync_metadata(
        &self,
        user_id: &str,
        metadata: SyncMetadataInput,
    ) -> Result<(), ExternalImportError> {
        let observed_at_epoch_seconds = self.clock.now_epoch_seconds();
        self.observations
            .upsert(ExternalObservation::new(ExternalObservationParams {
                user_id: user_id.to_string(),
                provider: metadata.provider.clone(),
                external_object_kind: metadata.external_object_kind,
                external_id: metadata.external_id.clone(),
                canonical_entity: metadata.canonical_entity.clone(),
                normalized_payload_hash: Some(metadata.normalized_payload_hash.clone()),
                dedup_key: metadata.dedup_key,
                observed_at_epoch_seconds,
            }))
            .await
            .map_err(map_repository_error)?;

        let sync_state = self
            .sync_states
            .find_by_provider_and_canonical_entity(
                user_id,
                metadata.provider.clone(),
                &metadata.canonical_entity,
            )
            .await
            .map_err(map_repository_error)?
            .unwrap_or_else(|| {
                ExternalSyncState::new(
                    user_id.to_string(),
                    metadata.provider.clone(),
                    metadata.canonical_entity.clone(),
                )
            });

        self.sync_states
            .upsert(sync_state.mark_synced(
                metadata.external_id,
                metadata.normalized_payload_hash,
                observed_at_epoch_seconds,
            ))
            .await
            .map_err(map_repository_error)?;

        Ok(())
    }
    async fn resolve_completed_workout_target(
        &self,
        incoming: CompletedWorkout,
        dedup_key: Option<&str>,
    ) -> Result<ResolvedCompletedWorkoutTarget, ExternalImportError> {
        let stored_workouts = self
            .completed_workouts
            .list_by_user_id(&incoming.user_id)
            .await
            .map_err(map_completed_workout_error)?;

        let direct_match = stored_workouts
            .iter()
            .find(|existing| existing.completed_workout_id == incoming.completed_workout_id)
            .cloned();
        if let Some(existing) = direct_match {
            return Ok(ResolvedCompletedWorkoutTarget {
                refresh_dates: completed_workout_refresh_dates(Some(&existing), &incoming),
                workout: merge_completed_workout(existing, incoming),
            });
        }

        let Some(dedup_key) = dedup_key else {
            return Ok(ResolvedCompletedWorkoutTarget {
                refresh_dates: completed_workout_refresh_dates(None, &incoming),
                workout: incoming,
            });
        };

        let observations = self
            .observations
            .find_by_dedup_key(
                &incoming.user_id,
                ExternalObjectKind::CompletedWorkout,
                dedup_key,
            )
            .await
            .map_err(map_repository_error)?;

        let mut matches = observations
            .into_iter()
            .filter(|observation| {
                observation.canonical_entity.entity_kind == CanonicalEntityKind::CompletedWorkout
            })
            .map(|observation| observation.canonical_entity.entity_id)
            .collect::<Vec<_>>();
        matches.sort();
        matches.dedup();

        match matches.as_slice() {
            [] => Ok(ResolvedCompletedWorkoutTarget {
                refresh_dates: completed_workout_refresh_dates(None, &incoming),
                workout: incoming,
            }),
            [canonical_id] => {
                let Some(existing) = stored_workouts
                    .into_iter()
                    .find(|workout| workout.completed_workout_id == *canonical_id)
                else {
                    return Err(ExternalImportError::CompletedWorkout(format!(
                        "stale completed workout dedup match for key '{dedup_key}' points to missing canonical workout '{canonical_id}'"
                    )));
                };

                Ok(ResolvedCompletedWorkoutTarget {
                    refresh_dates: completed_workout_refresh_dates(Some(&existing), &incoming),
                    workout: merge_completed_workout(existing, incoming),
                })
            }
            _ => Err(ExternalImportError::CompletedWorkout(format!(
                "ambiguous completed workout dedup match for key '{dedup_key}'"
            ))),
        }
    }

    async fn resolve_planned_workout_id_for_completed_workout(
        &self,
        user_id: &str,
        marker_sources: &[String],
        workout: &CompletedWorkout,
    ) -> Result<Option<ResolvedPlannedWorkoutLink>, ExternalImportError> {
        for source in marker_sources {
            let Some(match_token) = extract_planned_workout_marker(source) else {
                continue;
            };
            let planned_workout = self
                .planned_workout_tokens
                .find_by_match_token(user_id, &match_token)
                .await
                .map_err(map_planned_workout_token_error)?;
            if let Some(planned_workout) = planned_workout {
                return Ok(Some(ResolvedPlannedWorkoutLink {
                    planned_workout_id: planned_workout.planned_workout_id,
                    match_source: PlannedCompletedWorkoutLinkMatchSource::Token,
                }));
            }
        }

        let workout_date = date_key(&workout.start_date_local).to_string();
        let same_day_planned_workouts = self
            .planned_workouts
            .list_by_user_id_and_date_range(user_id, &workout_date, &workout_date)
            .await
            .map_err(map_planned_workout_error)?;
        let matching_name_planned_workouts = same_day_planned_workouts
            .into_iter()
            .filter(|planned_workout| {
                same_workout_name(planned_workout.name.as_deref(), workout.name.as_deref())
            })
            .collect::<Vec<_>>();

        match matching_name_planned_workouts.as_slice() {
            [] => Ok(None),
            [planned_workout] => Ok(Some(ResolvedPlannedWorkoutLink {
                planned_workout_id: planned_workout.planned_workout_id.clone(),
                match_source: PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            })),
            _ => Ok(None),
        }
    }
}

fn completed_workout_refresh_dates(
    existing: Option<&CompletedWorkout>,
    incoming: &CompletedWorkout,
) -> Vec<String> {
    let mut dates = vec![date_key(&incoming.start_date_local).to_string()];
    if let Some(existing) = existing {
        dates.push(date_key(&existing.start_date_local).to_string());
    }
    dates.sort();
    dates.dedup();
    dates
}

fn same_workout_name(left: Option<&str>, right: Option<&str>) -> bool {
    let Some(left) = normalize_workout_name(left) else {
        return false;
    };
    let Some(right) = normalize_workout_name(right) else {
        return false;
    };

    left == right
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

fn existing_link_candidate(
    existing_link: Option<&PlannedCompletedWorkoutLink>,
) -> Option<ResolvedPlannedWorkoutLink> {
    existing_link.map(|link| ResolvedPlannedWorkoutLink {
        planned_workout_id: link.planned_workout_id.clone(),
        match_source: link.match_source.clone(),
    })
}

async fn persist_legacy_planned_workout_link<PlannedCompletedLinks>(
    planned_completed_links: &PlannedCompletedLinks,
    workout: &CompletedWorkout,
    linked_at_epoch_seconds: i64,
) -> Result<(), ExternalImportError>
where
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository,
{
    let Some(planned_workout_id) = workout.planned_workout_id.as_ref() else {
        return Ok(());
    };

    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            workout.user_id.clone(),
            planned_workout_id.clone(),
            workout.completed_workout_id.clone(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            linked_at_epoch_seconds,
        ))
        .await
        .map_err(map_planned_completed_link_error)?;

    Ok(())
}

fn choose_preferred_planned_workout_link(
    existing_link: Option<ResolvedPlannedWorkoutLink>,
    resolved_link: Option<ResolvedPlannedWorkoutLink>,
) -> Option<ResolvedPlannedWorkoutLink> {
    match (existing_link, resolved_link) {
        (Some(existing), Some(resolved))
            if existing.planned_workout_id == resolved.planned_workout_id =>
        {
            Some(ResolvedPlannedWorkoutLink {
                planned_workout_id: existing.planned_workout_id,
                match_source: max_match_source(existing.match_source, resolved.match_source),
            })
        }
        (Some(existing), Some(resolved)) => {
            if match_source_rank(&resolved.match_source) > match_source_rank(&existing.match_source)
            {
                Some(resolved)
            } else {
                Some(existing)
            }
        }
        (Some(existing), None) => Some(existing),
        (None, Some(resolved)) => Some(resolved),
        (None, None) => None,
    }
}

fn max_match_source(
    left: PlannedCompletedWorkoutLinkMatchSource,
    right: PlannedCompletedWorkoutLinkMatchSource,
) -> PlannedCompletedWorkoutLinkMatchSource {
    if match_source_rank(&right) > match_source_rank(&left) {
        right
    } else {
        left
    }
}

fn match_source_rank(source: &PlannedCompletedWorkoutLinkMatchSource) -> u8 {
    match source {
        PlannedCompletedWorkoutLinkMatchSource::Heuristic => 0,
        PlannedCompletedWorkoutLinkMatchSource::Token => 1,
        PlannedCompletedWorkoutLinkMatchSource::Explicit => 2,
    }
}

impl<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
        Refresh,
    > ExternalImportUseCases
    for ExternalImportService<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        PlannedWorkoutTokens,
        PlannedCompletedLinks,
        Observations,
        SyncStates,
        Time,
        Refresh,
    >
where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    PlannedWorkoutTokens: PlannedWorkoutTokenRepository + Clone + 'static,
    PlannedCompletedLinks: PlannedCompletedWorkoutLinkRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    fn import(
        &self,
        command: ExternalImportCommand,
    ) -> BoxFuture<Result<ExternalImportOutcome, ExternalImportError>> {
        ExternalImportService::import(self, command)
    }
}

fn map_planned_workout_error(error: PlannedWorkoutError) -> ExternalImportError {
    match error {
        PlannedWorkoutError::Repository(message) => ExternalImportError::PlannedWorkout(message),
    }
}

fn map_completed_workout_error(error: CompletedWorkoutError) -> ExternalImportError {
    match error {
        CompletedWorkoutError::Repository(message) => {
            ExternalImportError::CompletedWorkout(message)
        }
    }
}

fn map_race_error(error: RaceError) -> ExternalImportError {
    match error {
        RaceError::NotFound => ExternalImportError::Race("race not found".to_string()),
        RaceError::Unauthenticated => {
            ExternalImportError::Race("race import unauthenticated".to_string())
        }
        RaceError::Validation(message)
        | RaceError::Unavailable(message)
        | RaceError::Internal(message) => ExternalImportError::Race(message),
    }
}

fn map_special_day_error(error: SpecialDayError) -> ExternalImportError {
    match error {
        SpecialDayError::Validation(message) | SpecialDayError::Repository(message) => {
            ExternalImportError::SpecialDay(message)
        }
    }
}

fn map_planned_workout_token_error(
    error: crate::domain::planned_workout_tokens::PlannedWorkoutTokenError,
) -> ExternalImportError {
    match error {
        crate::domain::planned_workout_tokens::PlannedWorkoutTokenError::Repository(message) => {
            ExternalImportError::Repository(message)
        }
    }
}

fn map_planned_completed_link_error(
    error: crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
) -> ExternalImportError {
    match error {
        crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError::Repository(
            message,
        ) => ExternalImportError::Repository(message),
    }
}
