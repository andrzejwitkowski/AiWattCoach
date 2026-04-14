mod completed_workout_dedup;
#[cfg(test)]
mod tests;

use tracing::warn;

use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    completed_workouts::{CompletedWorkout, CompletedWorkoutError, CompletedWorkoutRepository},
    identity::Clock,
    planned_workouts::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository},
    races::{Race, RaceError, RaceRepository},
    special_days::{SpecialDay, SpecialDayError, SpecialDayRepository},
};

use self::completed_workout_dedup::{completed_workout_dedup_key, merge_completed_workout};
use super::{
    BoxFuture, CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
    ExternalObservationParams, ExternalObservationRepository, ExternalProvider,
    ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStateRepository,
};

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
    Observations,
    SyncStates,
    Time,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    planned_workouts: PlannedWorkouts,
    completed_workouts: CompletedWorkouts,
    races: Races,
    special_days: SpecialDays,
    observations: Observations,
    sync_states: SyncStates,
    clock: Time,
    refresh: Refresh,
}

impl<PlannedWorkouts, CompletedWorkouts, Races, SpecialDays, Observations, SyncStates, Time>
    ExternalImportService<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
        Observations,
        SyncStates,
        Time,
    >
where
    PlannedWorkouts: PlannedWorkoutRepository + Clone + 'static,
    CompletedWorkouts: CompletedWorkoutRepository + Clone + 'static,
    Races: RaceRepository + Clone + 'static,
    SpecialDays: SpecialDayRepository + Clone + 'static,
    Observations: ExternalObservationRepository + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    pub fn new(
        planned_workouts: PlannedWorkouts,
        completed_workouts: CompletedWorkouts,
        races: Races,
        special_days: SpecialDays,
        observations: Observations,
        sync_states: SyncStates,
        clock: Time,
    ) -> Self {
        Self {
            planned_workouts,
            completed_workouts,
            races,
            special_days,
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
        self.persist_sync_metadata(
            &workout.user_id,
            SyncMetadataInput {
                provider: command.provider.clone(),
                external_object_kind: ExternalObjectKind::PlannedWorkout,
                external_id: command.external_id.clone(),
                canonical_entity: canonical_entity.clone(),
                normalized_payload_hash: command.normalized_payload_hash.clone(),
                dedup_key: None,
            },
        )
        .await?;
        self.refresh_imported_range(&workout.user_id, &workout.date, &workout.date)
            .await;

        Ok(ExternalImportOutcome {
            canonical_entity,
            provider: command.provider,
            external_id: command.external_id,
        })
    }

    async fn import_completed_workout(
        &self,
        command: ExternalCompletedWorkoutImport,
    ) -> Result<ExternalImportOutcome, ExternalImportError> {
        let dedup_key = completed_workout_dedup_key(&command.workout);
        let workout = self
            .resolve_completed_workout_target(command.workout, dedup_key.as_deref())
            .await?;
        let workout = self
            .completed_workouts
            .upsert(workout)
            .await
            .map_err(map_completed_workout_error)?;
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            workout.completed_workout_id.clone(),
        );
        self.persist_sync_metadata(
            &workout.user_id,
            SyncMetadataInput {
                provider: command.provider.clone(),
                external_object_kind: ExternalObjectKind::CompletedWorkout,
                external_id: command.external_id.clone(),
                canonical_entity: canonical_entity.clone(),
                normalized_payload_hash: command.normalized_payload_hash.clone(),
                dedup_key,
            },
        )
        .await?;

        let date = workout
            .start_date_local
            .get(..10)
            .unwrap_or(&workout.start_date_local);
        self.refresh_imported_range(&workout.user_id, date, date)
            .await;

        Ok(ExternalImportOutcome {
            canonical_entity,
            provider: command.provider,
            external_id: command.external_id,
        })
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
        self.persist_sync_metadata(
            &race.user_id,
            SyncMetadataInput {
                provider: command.provider.clone(),
                external_object_kind: ExternalObjectKind::Race,
                external_id: command.external_id.clone(),
                canonical_entity: canonical_entity.clone(),
                normalized_payload_hash: command.normalized_payload_hash.clone(),
                dedup_key: None,
            },
        )
        .await?;
        self.refresh_imported_range(&race.user_id, &race.date, &race.date)
            .await;

        Ok(ExternalImportOutcome {
            canonical_entity,
            provider: command.provider,
            external_id: command.external_id,
        })
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
        self.persist_sync_metadata(
            &special_day.user_id,
            SyncMetadataInput {
                provider: command.provider.clone(),
                external_object_kind: ExternalObjectKind::SpecialDay,
                external_id: command.external_id.clone(),
                canonical_entity: canonical_entity.clone(),
                normalized_payload_hash: command.normalized_payload_hash.clone(),
                dedup_key: None,
            },
        )
        .await?;
        self.refresh_imported_range(&special_day.user_id, &special_day.date, &special_day.date)
            .await;

        Ok(ExternalImportOutcome {
            canonical_entity,
            provider: command.provider,
            external_id: command.external_id,
        })
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

    async fn refresh_imported_range(&self, user_id: &str, oldest: &str, newest: &str) {
        if let Err(error) = self
            .refresh
            .refresh_range_for_user(user_id, oldest, newest)
            .await
        {
            warn!(
                %user_id,
                %oldest,
                %newest,
                %error,
                "external import succeeded but calendar view refresh failed"
            );
        }
    }

    async fn resolve_completed_workout_target(
        &self,
        incoming: CompletedWorkout,
        dedup_key: Option<&str>,
    ) -> Result<CompletedWorkout, ExternalImportError> {
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
            return Ok(merge_completed_workout(existing, incoming));
        }

        let Some(dedup_key) = dedup_key else {
            return Ok(incoming);
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
            [] => Ok(incoming),
            [canonical_id] => {
                let Some(existing) = stored_workouts
                    .into_iter()
                    .find(|workout| workout.completed_workout_id == *canonical_id)
                else {
                    return Ok(incoming);
                };

                Ok(merge_completed_workout(existing, incoming))
            }
            _ => Err(ExternalImportError::CompletedWorkout(format!(
                "ambiguous completed workout dedup match for key '{dedup_key}'"
            ))),
        }
    }
}

impl<
        PlannedWorkouts,
        CompletedWorkouts,
        Races,
        SpecialDays,
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
        SpecialDayError::Repository(message) => ExternalImportError::SpecialDay(message),
    }
}

fn map_repository_error(error: ExternalSyncRepositoryError) -> ExternalImportError {
    match error {
        ExternalSyncRepositoryError::Storage(message)
        | ExternalSyncRepositoryError::CorruptData(message) => {
            ExternalImportError::Repository(message)
        }
    }
}
