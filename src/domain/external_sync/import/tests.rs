use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar_view::{
        BoxFuture as RefreshBoxFuture, CalendarEntryView, CalendarEntryViewError,
        CalendarEntryViewRefreshPort,
    },
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutError,
        CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics, CompletedWorkoutRepository,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    identity::Clock,
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
        PlannedWorkoutRepository, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
        PlannedWorkoutText,
    },
    races::{Race, RaceDiscipline, RaceError, RacePriority, RaceRepository},
    special_days::{SpecialDay, SpecialDayError, SpecialDayKind, SpecialDayRepository},
};

use super::{
    completed_workout_dedup::completed_workout_dedup_key, ExternalCompletedWorkoutImport,
    ExternalImportCommand, ExternalImportError, ExternalImportService,
    ExternalPlannedWorkoutImport, ExternalRaceImport, ExternalSpecialDayImport,
};
use crate::domain::external_sync::{
    CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
    ExternalObservationParams, ExternalObservationRepository, ExternalProvider,
    ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStateRepository,
};

#[tokio::test]
async fn import_planned_workout_persists_canonical_state_and_refreshes_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = ExternalImportService::new(
        planned_workouts.clone(),
        completed_workouts,
        races,
        special_days,
        observations.clone(),
        sync_states.clone(),
        FixedClock,
    )
    .with_calendar_view_refresh(refresh.clone());

    let outcome = service
        .import(ExternalImportCommand::UpsertPlannedWorkout(
            ExternalPlannedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-event-144".to_string(),
                normalized_payload_hash: "hash-planned-1".to_string(),
                workout: sample_planned_workout(),
            },
        ))
        .await
        .unwrap();

    let stored = planned_workouts
        .list_by_user_id_and_date_range("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].planned_workout_id, "planned-imported-1");
    assert_eq!(observations.stored().len(), 1);
    assert_eq!(sync_states.stored().len(), 1);
    assert_eq!(
        outcome.canonical_entity,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "planned-imported-1".to_string(),
        )
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
}

#[tokio::test]
async fn import_completed_workout_persists_canonical_state_and_refreshes_start_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = ExternalImportService::new(
        planned_workouts,
        completed_workouts.clone(),
        races,
        special_days,
        observations.clone(),
        sync_states.clone(),
        FixedClock,
    )
    .with_calendar_view_refresh(refresh.clone());

    let outcome = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-1".to_string(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts
        .list_by_user_id_and_date_range("user-1", "2026-05-11", "2026-05-11")
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].completed_workout_id, "completed-imported-1");
    assert_eq!(observations.stored().len(), 1);
    assert_eq!(sync_states.stored().len(), 1);
    assert_eq!(
        outcome.canonical_entity,
        CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "completed-imported-1".to_string(),
        )
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-11".to_string(),
            "2026-05-11".to_string(),
        )]
    );
}

#[tokio::test]
async fn import_completed_workout_reuses_existing_canonical_workout_for_matching_dedup_key() {
    let observations = InMemoryObservationRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let existing = sample_completed_workout();
    completed_workouts.upsert(existing.clone()).await.unwrap();
    observations
        .upsert(ExternalObservation::new(ExternalObservationParams {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            external_object_kind: ExternalObjectKind::CompletedWorkout,
            external_id: "intervals-activity-77".to_string(),
            canonical_entity: CanonicalEntityRef::new(
                CanonicalEntityKind::CompletedWorkout,
                existing.completed_workout_id.clone(),
            ),
            normalized_payload_hash: Some("hash-existing".to_string()),
            dedup_key: completed_workout_dedup_key(&existing),
            observed_at_epoch_seconds: 1_699_999_000,
        }))
        .await
        .unwrap();
    let service = ExternalImportService::new(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        observations.clone(),
        InMemorySyncStateRepository::default(),
        FixedClock,
    );

    let incoming = sample_completed_workout_for_provider(
        ExternalProvider::Wahoo,
        "wahoo-activity-1",
        Some(serde_json::json!([180, 240, 310, 330])),
    );
    let outcome = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Wahoo,
                external_id: "wahoo-activity-1".to_string(),
                normalized_payload_hash: "hash-wahoo-1".to_string(),
                workout: incoming,
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].completed_workout_id,
        existing.completed_workout_id
    );
    assert_eq!(
        outcome.canonical_entity.entity_id,
        existing.completed_workout_id
    );
    assert_eq!(observations.stored().len(), 2);
}

#[tokio::test]
async fn import_completed_workout_matches_even_when_other_provider_arrives_first() {
    let service = ExternalImportService::new(
        InMemoryPlannedWorkoutRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
        FixedClock,
    );

    let first = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Wahoo,
                external_id: "wahoo-activity-1".to_string(),
                normalized_payload_hash: "hash-wahoo-1".to_string(),
                workout: sample_completed_workout_for_provider(
                    ExternalProvider::Wahoo,
                    "wahoo-activity-1",
                    None,
                ),
            },
        )))
        .await
        .unwrap();
    let second = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-intervals-1".to_string(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    assert_eq!(
        first.canonical_entity.entity_id,
        second.canonical_entity.entity_id
    );
}

#[tokio::test]
async fn import_completed_workout_uses_fingerprint_when_external_ids_do_not_help() {
    let service = ExternalImportService::new(
        InMemoryPlannedWorkoutRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
        FixedClock,
    );

    let first = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-intervals-1".to_string(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();
    let second = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Other,
                external_id: "provider-b-77".to_string(),
                normalized_payload_hash: "hash-provider-b-1".to_string(),
                workout: sample_completed_workout_for_provider(
                    ExternalProvider::Other,
                    "provider-b-77",
                    None,
                ),
            },
        )))
        .await
        .unwrap();

    assert_eq!(
        first.canonical_entity.entity_id,
        second.canonical_entity.entity_id
    );
}

#[tokio::test]
async fn import_completed_workout_rejects_ambiguous_fingerprint_match() {
    let observations = InMemoryObservationRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let first = sample_completed_workout_with_id("completed-a");
    let second = sample_completed_workout_with_id("completed-b");
    let dedup_key = completed_workout_dedup_key(&first).unwrap();
    completed_workouts.upsert(first.clone()).await.unwrap();
    completed_workouts.upsert(second.clone()).await.unwrap();
    for (provider, external_id, canonical_id) in [
        (
            ExternalProvider::Intervals,
            "intervals-a",
            first.completed_workout_id.clone(),
        ),
        (
            ExternalProvider::Wahoo,
            "wahoo-b",
            second.completed_workout_id.clone(),
        ),
    ] {
        observations
            .upsert(ExternalObservation::new(ExternalObservationParams {
                user_id: "user-1".to_string(),
                provider,
                external_object_kind: ExternalObjectKind::CompletedWorkout,
                external_id: external_id.to_string(),
                canonical_entity: CanonicalEntityRef::new(
                    CanonicalEntityKind::CompletedWorkout,
                    canonical_id,
                ),
                normalized_payload_hash: Some("hash-existing".to_string()),
                dedup_key: Some(dedup_key.clone()),
                observed_at_epoch_seconds: 1_699_999_000,
            }))
            .await
            .unwrap();
    }
    let service = ExternalImportService::new(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts,
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        observations,
        InMemorySyncStateRepository::default(),
        FixedClock,
    );

    let error = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Other,
                external_id: "provider-c-1".to_string(),
                normalized_payload_hash: "hash-provider-c-1".to_string(),
                workout: sample_completed_workout_for_provider(
                    ExternalProvider::Other,
                    "provider-c-1",
                    None,
                ),
            },
        )))
        .await
        .unwrap_err();

    assert!(
        matches!(error, ExternalImportError::CompletedWorkout(message) if message.contains("ambiguous"))
    );
}

#[tokio::test]
async fn import_race_persists_canonical_state_and_refreshes_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = ExternalImportService::new(
        planned_workouts,
        completed_workouts,
        races.clone(),
        special_days,
        observations.clone(),
        sync_states.clone(),
        FixedClock,
    )
    .with_calendar_view_refresh(refresh.clone());

    let outcome = service
        .import(ExternalImportCommand::UpsertRace(ExternalRaceImport {
            provider: ExternalProvider::Intervals,
            external_id: "race-44".to_string(),
            normalized_payload_hash: "hash-race-1".to_string(),
            race: Race {
                race_id: "race-imported-1".to_string(),
                user_id: "user-1".to_string(),
                date: "2026-09-12".to_string(),
                name: "Imported Race".to_string(),
                distance_meters: 120_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::B,
                result: None,
                created_at_epoch_seconds: 0,
                updated_at_epoch_seconds: 0,
            },
        }))
        .await
        .unwrap();

    assert_eq!(races.list_by_user_id("user-1").await.unwrap().len(), 1);
    assert_eq!(observations.stored().len(), 1);
    assert_eq!(sync_states.stored().len(), 1);
    assert_eq!(
        outcome.canonical_entity,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-imported-1".to_string())
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-09-12".to_string(),
            "2026-09-12".to_string(),
        )]
    );
}

#[tokio::test]
async fn import_special_day_persists_canonical_state_and_refreshes_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = ExternalImportService::new(
        planned_workouts,
        completed_workouts,
        races,
        special_days.clone(),
        observations.clone(),
        sync_states.clone(),
        FixedClock,
    )
    .with_calendar_view_refresh(refresh.clone());

    let outcome = service
        .import(ExternalImportCommand::UpsertSpecialDay(
            ExternalSpecialDayImport {
                provider: ExternalProvider::Intervals,
                external_id: "special-55".to_string(),
                normalized_payload_hash: "hash-special-1".to_string(),
                special_day: SpecialDay::new(
                    "special-imported-1".to_string(),
                    "user-1".to_string(),
                    "2026-06-01".to_string(),
                    SpecialDayKind::Note,
                ),
            },
        ))
        .await
        .unwrap();

    assert_eq!(
        special_days.list_by_user_id("user-1").await.unwrap().len(),
        1
    );
    assert_eq!(observations.stored().len(), 1);
    assert_eq!(sync_states.stored().len(), 1);
    assert_eq!(
        outcome.canonical_entity,
        CanonicalEntityRef::new(
            CanonicalEntityKind::SpecialDay,
            "special-imported-1".to_string(),
        )
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-06-01".to_string(),
            "2026-06-01".to_string(),
        )]
    );
}

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
struct InMemoryObservationRepository {
    stored: Arc<Mutex<Vec<ExternalObservation>>>,
}

#[derive(Clone, Default)]
struct InMemoryPlannedWorkoutRepository {
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
struct InMemoryCompletedWorkoutRepository {
    stored: Arc<Mutex<Vec<CompletedWorkout>>>,
}

#[derive(Clone, Default)]
struct InMemoryRaceRepository {
    stored: Arc<Mutex<Vec<Race>>>,
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
struct InMemorySpecialDayRepository {
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

impl InMemoryObservationRepository {
    fn stored(&self) -> Vec<ExternalObservation> {
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
        external_object_kind: ExternalObjectKind,
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
struct InMemorySyncStateRepository {
    stored: Arc<Mutex<Vec<ExternalSyncState>>>,
}

impl InMemorySyncStateRepository {
    fn stored(&self) -> Vec<ExternalSyncState> {
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
struct RecordingRefresh {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
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

fn sample_planned_workout() -> PlannedWorkout {
    PlannedWorkout::new(
        "planned-imported-1".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
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
}

fn sample_completed_workout() -> CompletedWorkout {
    sample_completed_workout_with_id("completed-imported-1")
}

fn sample_completed_workout_with_id(completed_workout_id: &str) -> CompletedWorkout {
    sample_completed_workout_for_provider(
        ExternalProvider::Intervals,
        completed_workout_id,
        Some(serde_json::json!([180, 240, 310])),
    )
}

fn sample_completed_workout_for_provider(
    _provider: ExternalProvider,
    completed_workout_id: &str,
    primary_series: Option<serde_json::Value>,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        "user-1".to_string(),
        "2026-05-11T08:00:00".to_string(),
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
