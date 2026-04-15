use crate::domain::{
    completed_workouts::{CompletedWorkoutRepository, CompletedWorkoutSeries},
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalImportCommand, ExternalObjectKind,
        ExternalObservation, ExternalObservationParams, ExternalObservationRepository,
        ExternalProvider,
    },
    planned_completed_links::{
        PlannedCompletedWorkoutLinkMatchSource, PlannedCompletedWorkoutLinkRepository,
    },
    planned_workout_tokens::{PlannedWorkoutToken, PlannedWorkoutTokenRepository},
    planned_workouts::PlannedWorkoutRepository,
};

use super::super::{
    completed_workout_dedup::completed_workout_dedup_key, ExternalCompletedWorkoutImport,
    ExternalImportError,
};
use super::support::*;

#[tokio::test]
async fn import_completed_workout_persists_canonical_state_and_refreshes_start_day() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let races = InMemoryRaceRepository::default();
    let special_days = InMemorySpecialDayRepository::default();
    let planned_workout_tokens = InMemoryPlannedWorkoutTokenRepository::default();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    let observations = InMemoryObservationRepository::default();
    let sync_states = InMemorySyncStateRepository::default();
    let refresh = RecordingRefresh::default();
    let service = external_import_service(
        planned_workouts,
        completed_workouts.clone(),
        races,
        special_days,
        planned_workout_tokens,
        planned_completed_links,
        observations.clone(),
        sync_states.clone(),
        refresh.clone(),
    );

    let outcome = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-1".to_string(),
                marker_sources: Vec::new(),
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
    let service = external_import_service_without_refresh(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        observations.clone(),
        InMemorySyncStateRepository::default(),
    );

    let incoming = sample_completed_workout_for_provider(
        ExternalProvider::Wahoo,
        "wahoo-activity-1",
        Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310, 330])),
    );
    let outcome = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Wahoo,
                external_id: "wahoo-activity-1".to_string(),
                normalized_payload_hash: "hash-wahoo-1".to_string(),
                marker_sources: Vec::new(),
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
    let service = external_import_service_without_refresh(
        InMemoryPlannedWorkoutRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    let first = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Wahoo,
                external_id: "wahoo-activity-1".to_string(),
                normalized_payload_hash: "hash-wahoo-1".to_string(),
                marker_sources: Vec::new(),
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
                marker_sources: Vec::new(),
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
    let service = external_import_service_without_refresh(
        InMemoryPlannedWorkoutRepository::default(),
        InMemoryCompletedWorkoutRepository::default(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    let first = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-intervals-1".to_string(),
                marker_sources: Vec::new(),
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
                marker_sources: Vec::new(),
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
    let service = external_import_service_without_refresh(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts,
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        observations,
        InMemorySyncStateRepository::default(),
    );

    let error = service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Other,
                external_id: "provider-c-1".to_string(),
                normalized_payload_hash: "hash-provider-c-1".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout_for_provider(
                    ExternalProvider::Other,
                    "provider-c-1",
                    None,
                ),
            },
        )))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ExternalImportError::CompletedWorkout(message) if message.contains("ambiguous")
    ));
}

#[tokio::test]
async fn import_completed_workout_links_by_match_token_and_persists_link() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout())
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let planned_workout_tokens = InMemoryPlannedWorkoutTokenRepository::default();
    planned_workout_tokens
        .upsert(PlannedWorkoutToken::new(
            "user-1".to_string(),
            "planned-imported-1".to_string(),
            "PW123ABC45".to_string(),
        ))
        .await
        .unwrap();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        planned_workout_tokens,
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-1".to_string(),
                marker_sources: vec!["Warmup notes\n[AIWATTCOACH:pw=PW123ABC45]".to_string()],
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-imported-1")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-imported-1");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Token
    );
}
