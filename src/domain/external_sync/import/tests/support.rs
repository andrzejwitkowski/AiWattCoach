use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar_view::{
        BoxFuture as RefreshBoxFuture, CalendarEntryView, CalendarEntryViewError,
        CalendarEntryViewRefreshPort,
    },
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutError,
        CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics, CompletedWorkoutRepository,
        CompletedWorkoutSeries, CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    external_sync::{
        CanonicalEntityRef, ExternalObservation, ExternalObservationRepository, ExternalProvider,
        ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStateRepository,
    },
    identity::Clock,
    planned_completed_links::{
        PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError,
        PlannedCompletedWorkoutLinkRepository,
    },
    planned_workout_tokens::{
        PlannedWorkoutToken, PlannedWorkoutTokenError, PlannedWorkoutTokenRepository,
    },
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
        PlannedWorkoutRepository, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
        PlannedWorkoutText,
    },
    races::{Race, RaceError, RaceRepository},
    special_days::{SpecialDay, SpecialDayError, SpecialDayRepository},
};

use super::super::ExternalImportService;

#[derive(Clone)]
pub(super) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryObservationRepository {
    stored: Arc<Mutex<Vec<ExternalObservation>>>,
}

#[derive(Clone, Default)]
pub(super) struct InMemoryPlannedWorkoutRepository {
    stored: Arc<Mutex<Vec<PlannedWorkout>>>,
}

impl PlannedWorkoutRepository for InMemoryPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>
    {
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
    ) -> crate::domain::planned_workouts::BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>>
    {
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
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> crate::domain::planned_workouts::BoxFuture<Result<PlannedWorkout, PlannedWorkoutError>>
    {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.planned_workout_id == workout.planned_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

#[derive(Clone, Default)]
pub(super) struct InMemoryRaceRepository {
    stored: Arc<Mutex<Vec<Race>>>,
}

#[derive(Clone, Default)]
pub(super) struct InMemoryPlannedWorkoutTokenRepository {
    stored: Arc<Mutex<Vec<PlannedWorkoutToken>>>,
}

#[derive(Clone, Default)]
pub(super) struct InMemoryPlannedCompletedWorkoutLinkRepository {
    stored: Arc<Mutex<Vec<PlannedCompletedWorkoutLink>>>,
}

impl RaceRepository for InMemoryRaceRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
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
        range: &crate::domain::intervals::DateRange,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(stored
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
    ) -> crate::domain::races::BoxFuture<Result<Option<Race>, RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|race| race.user_id == user_id && race.race_id == race_id)
                .cloned())
        })
    }

    fn upsert(&self, race: Race) -> crate::domain::races::BoxFuture<Result<Race, RaceError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == race.user_id && existing.race_id == race.race_id)
            });
            stored.push(race.clone());
            Ok(race)
        })
    }

    fn delete(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<(), RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            stored
                .lock()
                .unwrap()
                .retain(|race| !(race.user_id == user_id && race.race_id == race_id));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemorySpecialDayRepository {
    stored: Arc<Mutex<Vec<SpecialDay>>>,
}

impl SpecialDayRepository for InMemorySpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::special_days::BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|day| day.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::special_days::BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> crate::domain::special_days::BoxFuture<Result<SpecialDay, SpecialDayError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == special_day.user_id
                    && existing.special_day_id == special_day.special_day_id)
            });
            stored.push(special_day.clone());
            Ok(special_day)
        })
    }
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
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

impl PlannedWorkoutTokenRepository for InMemoryPlannedWorkoutTokenRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|token| {
                    token.user_id == user_id && token.planned_workout_id == planned_workout_id
                })
                .cloned())
        })
    }

    fn find_by_match_token(
        &self,
        user_id: &str,
        match_token: &str,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let match_token = match_token.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|token| token.user_id == user_id && token.match_token == match_token)
                .cloned())
        })
    }

    fn upsert(
        &self,
        token: PlannedWorkoutToken,
    ) -> crate::domain::planned_workout_tokens::BoxFuture<
        Result<PlannedWorkoutToken, PlannedWorkoutTokenError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == token.user_id
                    && existing.planned_workout_id == token.planned_workout_id)
            });
            stored.push(token.clone());
            Ok(token)
        })
    }
}

impl PlannedCompletedWorkoutLinkRepository for InMemoryPlannedCompletedWorkoutLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|link| {
                    link.user_id == user_id && link.planned_workout_id == planned_workout_id
                })
                .cloned())
        })
    }

    fn find_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|link| {
                    link.user_id == user_id && link.completed_workout_id == completed_workout_id
                })
                .cloned())
        })
    }

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == link.user_id
                    && (existing.planned_workout_id == link.planned_workout_id
                        || existing.completed_workout_id == link.completed_workout_id))
            });
            stored.push(link.clone());
            Ok(link)
        })
    }
}

impl InMemoryObservationRepository {
    pub(super) fn stored(&self) -> Vec<ExternalObservation> {
        self.stored.lock().unwrap().clone()
    }
}

impl ExternalObservationRepository for InMemoryObservationRepository {
    fn upsert(
        &self,
        observation: ExternalObservation,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalObservation, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == observation.user_id
                    && existing.provider == observation.provider
                    && existing.external_id == observation.external_id)
            });
            stored.push(observation.clone());
            Ok(observation)
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalObservation>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|observation| {
                    observation.user_id == user_id
                        && observation.provider == provider
                        && observation.external_id == external_id
                })
                .cloned())
        })
    }

    fn find_by_dedup_key(
        &self,
        user_id: &str,
        external_object_kind: crate::domain::external_sync::ExternalObjectKind,
        dedup_key: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalObservation>, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let dedup_key = dedup_key.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|observation| {
                    observation.user_id == user_id
                        && observation.external_object_kind == external_object_kind
                        && observation.dedup_key.as_deref() == Some(dedup_key.as_str())
                })
                .cloned()
                .collect())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemorySyncStateRepository {
    stored: Arc<Mutex<Vec<ExternalSyncState>>>,
}

impl InMemorySyncStateRepository {
    pub(super) fn stored(&self) -> Vec<ExternalSyncState> {
        self.stored.lock().unwrap().clone()
    }
}

impl ExternalSyncStateRepository for InMemorySyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.canonical_entity == state.canonical_entity)
            });
            stored.push(state.clone());
            Ok(state)
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
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            Ok(stored
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
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            Ok(stored
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
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            stored.lock().unwrap().retain(|state| {
                !(state.user_id == user_id
                    && state.provider == provider
                    && state.canonical_entity == canonical_entity)
            });
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingRefresh {
    pub(super) fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> RefreshBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
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

#[expect(
    clippy::too_many_arguments,
    reason = "Test wiring mirrors ExternalImportService dependencies explicitly"
)]
pub(super) fn external_import_service(
    planned_workouts: InMemoryPlannedWorkoutRepository,
    completed_workouts: InMemoryCompletedWorkoutRepository,
    races: InMemoryRaceRepository,
    special_days: InMemorySpecialDayRepository,
    planned_workout_tokens: InMemoryPlannedWorkoutTokenRepository,
    planned_completed_links: InMemoryPlannedCompletedWorkoutLinkRepository,
    observations: InMemoryObservationRepository,
    sync_states: InMemorySyncStateRepository,
    refresh: RecordingRefresh,
) -> ExternalImportService<
    InMemoryPlannedWorkoutRepository,
    InMemoryCompletedWorkoutRepository,
    InMemoryRaceRepository,
    InMemorySpecialDayRepository,
    InMemoryPlannedWorkoutTokenRepository,
    InMemoryPlannedCompletedWorkoutLinkRepository,
    InMemoryObservationRepository,
    InMemorySyncStateRepository,
    FixedClock,
    RecordingRefresh,
> {
    ExternalImportService::new(
        planned_workouts,
        completed_workouts,
        races,
        special_days,
        planned_workout_tokens,
        planned_completed_links,
        observations,
        sync_states,
        FixedClock,
    )
    .with_calendar_view_refresh(refresh)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Test wiring mirrors ExternalImportService dependencies explicitly"
)]
pub(super) fn external_import_service_without_refresh(
    planned_workouts: InMemoryPlannedWorkoutRepository,
    completed_workouts: InMemoryCompletedWorkoutRepository,
    races: InMemoryRaceRepository,
    special_days: InMemorySpecialDayRepository,
    planned_workout_tokens: InMemoryPlannedWorkoutTokenRepository,
    planned_completed_links: InMemoryPlannedCompletedWorkoutLinkRepository,
    observations: InMemoryObservationRepository,
    sync_states: InMemorySyncStateRepository,
) -> ExternalImportService<
    InMemoryPlannedWorkoutRepository,
    InMemoryCompletedWorkoutRepository,
    InMemoryRaceRepository,
    InMemorySpecialDayRepository,
    InMemoryPlannedWorkoutTokenRepository,
    InMemoryPlannedCompletedWorkoutLinkRepository,
    InMemoryObservationRepository,
    InMemorySyncStateRepository,
    FixedClock,
> {
    ExternalImportService::new(
        planned_workouts,
        completed_workouts,
        races,
        special_days,
        planned_workout_tokens,
        planned_completed_links,
        observations,
        sync_states,
        FixedClock,
    )
}

pub(super) fn sample_planned_workout() -> PlannedWorkout {
    sample_planned_workout_on_date("planned-imported-1", "2026-05-10")
}

pub(super) fn sample_planned_workout_on_date(
    planned_workout_id: &str,
    date: &str,
) -> PlannedWorkout {
    PlannedWorkout::new(
        planned_workout_id.to_string(),
        "user-1".to_string(),
        date.to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Imported Threshold".to_string(),
                }),
                PlannedWorkoutLine::Step(PlannedWorkoutStep {
                    duration_seconds: 900,
                    kind: PlannedWorkoutStepKind::Steady,
                    target: PlannedWorkoutTarget::PercentFtp {
                        min: 90.0,
                        max: 95.0,
                    },
                }),
            ],
        },
    )
    .with_event_metadata(
        Some("Imported Threshold".to_string()),
        Some("Strong over-unders".to_string()),
        Some("Ride".to_string()),
    )
}

pub(super) fn sample_completed_workout() -> CompletedWorkout {
    sample_completed_workout_with_id("completed-imported-1")
}

pub(super) fn sample_completed_workout_with_id(completed_workout_id: &str) -> CompletedWorkout {
    sample_completed_workout_for_provider(
        ExternalProvider::Intervals,
        completed_workout_id,
        Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
    )
}

pub(super) fn sample_completed_workout_for_provider(
    _provider: ExternalProvider,
    completed_workout_id: &str,
    primary_series: Option<CompletedWorkoutSeries>,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        "user-1".to_string(),
        "2026-05-11T08:00:00".to_string(),
        None,
        Some("Threshold Ride".to_string()),
        Some("Strong day".to_string()),
        Some("Ride".to_string()),
        Some(3600),
        Some(35_200.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(78),
            normalized_power_watts: Some(245),
            intensity_factor: Some(0.83),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(221),
            ftp_watts: Some(295),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: vec![CompletedWorkoutIntervalGroup {
                id: "group-1".to_string(),
                count: Some(1),
                start_index: Some(0),
                moving_time_seconds: Some(3600),
                elapsed_time_seconds: Some(3660),
                distance_meters: Some(35200.0),
                average_power_watts: Some(221),
                normalized_power_watts: Some(245),
                training_stress_score: Some(78.0),
                average_heart_rate_bpm: Some(150),
                average_cadence_rpm: Some(88.0),
                average_speed_mps: None,
                average_stride_meters: None,
            }],
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series,
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z3".to_string(),
                seconds: 1200,
            }],
            heart_rate_zone_times: vec![600],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    )
}
