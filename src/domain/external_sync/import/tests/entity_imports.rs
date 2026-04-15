use crate::domain::{
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalImportCommand, ExternalProvider,
    },
    planned_workouts::PlannedWorkoutRepository,
    races::{Race, RaceDiscipline, RacePriority, RaceRepository},
    special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
};

use super::super::{ExternalPlannedWorkoutImport, ExternalRaceImport, ExternalSpecialDayImport};
use super::support::*;

#[tokio::test]
async fn import_planned_workout_persists_canonical_state_and_refreshes_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = external_import_service(
        planned_workouts.clone(),
        completed_workouts,
        races,
        special_days,
        observations.clone(),
        sync_states.clone(),
        refresh.clone(),
    );

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
async fn import_race_persists_canonical_state_and_refreshes_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = external_import_service(
        planned_workouts,
        completed_workouts,
        races.clone(),
        special_days,
        observations.clone(),
        sync_states.clone(),
        refresh.clone(),
    );

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
    let service = external_import_service(
        planned_workouts,
        completed_workouts,
        races,
        special_days.clone(),
        observations.clone(),
        sync_states.clone(),
        refresh.clone(),
    );

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
