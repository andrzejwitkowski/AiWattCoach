use crate::domain::{
    completed_workouts::{CompletedWorkoutRepository, CompletedWorkoutSeries},
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalImportCommand, ExternalObjectKind,
        ExternalObservation, ExternalObservationParams, ExternalObservationRepository,
        ExternalProvider,
    },
    planned_completed_links::{
        PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
        PlannedCompletedWorkoutLinkRepository,
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
async fn import_completed_workout_returns_error_for_stale_dedup_mapping() {
    let observations = InMemoryObservationRepository::default();
    let existing = sample_completed_workout();
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
        InMemoryCompletedWorkoutRepository::default(),
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
                provider: ExternalProvider::Wahoo,
                external_id: "wahoo-activity-1".to_string(),
                normalized_payload_hash: "hash-wahoo-1".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout_for_provider(
                    ExternalProvider::Wahoo,
                    "wahoo-activity-1",
                    Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310, 330])),
                ),
            },
        )))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ExternalImportError::CompletedWorkout(message) if message.contains("stale completed workout dedup match")
    ));
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
async fn import_completed_workout_refreshes_old_and_new_dates_when_existing_canonical_date_moves() {
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let refresh = RecordingRefresh::default();
    let mut existing = sample_completed_workout();
    existing.start_date_local = "2026-05-10T08:00:00".to_string();
    completed_workouts.upsert(existing.clone()).await.unwrap();
    let service = external_import_service(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
        refresh.clone(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-intervals-2".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].start_date_local, "2026-05-11T08:00:00");
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-11".to_string(),
        )]
    );
}

#[tokio::test]
async fn import_completed_workout_merges_enriched_details_into_existing_sparse_record() {
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let mut existing = sample_completed_workout();
    existing.details.streams.clear();
    existing.details.intervals.clear();
    existing.details.interval_groups.clear();
    completed_workouts.upsert(existing.clone()).await.unwrap();

    let service = external_import_service_without_refresh(
        InMemoryPlannedWorkoutRepository::default(),
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        InMemoryPlannedCompletedWorkoutLinkRepository::default(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    let mut incoming = sample_completed_workout();
    incoming
        .details
        .streams
        .push(crate::domain::completed_workouts::CompletedWorkoutStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310, 330])),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        });

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-enriched-details".to_string(),
                marker_sources: Vec::new(),
                workout: incoming,
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(stored.len(), 1);
    assert!(!stored[0].details.streams.is_empty());
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
            "AB123CDE45".to_string(),
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
                marker_sources: vec!["Warmup notes\n[AIWATTCOACH:pw=AB123CDE45]".to_string()],
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

#[tokio::test]
async fn import_completed_workout_links_single_same_day_planned_workout_as_heuristic() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-same-day",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-heuristic".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-same-day")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-same-day");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
}

#[tokio::test]
async fn import_completed_workout_prefers_token_match_source_over_same_day_fallback() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-token",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let planned_workout_tokens = InMemoryPlannedWorkoutTokenRepository::default();
    planned_workout_tokens
        .upsert(PlannedWorkoutToken::new(
            "user-1".to_string(),
            "planned-token".to_string(),
            "AB123CDE45".to_string(),
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
                normalized_payload_hash: "hash-completed-token-priority".to_string(),
                marker_sources: vec!["Warmup notes\n[AIWATTCOACH:pw=AB123CDE45]".to_string()],
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-token")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-token");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Token
    );
}

#[tokio::test]
async fn import_completed_workout_does_not_downgrade_existing_explicit_link_match_source() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-same-day",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let mut existing = sample_completed_workout();
    existing.planned_workout_id = Some("planned-same-day".to_string());
    completed_workouts.upsert(existing).await.unwrap();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-same-day".to_string(),
            "completed-imported-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_000,
        ))
        .await
        .unwrap();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts,
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-explicit".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Explicit
    );
}

#[tokio::test]
async fn import_completed_workout_does_not_inherit_explicit_match_source_from_different_pair() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-original",
            "2026-05-10",
        ))
        .await
        .unwrap();
    planned_workouts
        .upsert(sample_planned_workout_on_date("planned-new", "2026-05-11"))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-original".to_string(),
            "completed-old".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_000,
        ))
        .await
        .unwrap();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts,
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-new-pair".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-new");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
}

#[tokio::test]
async fn import_completed_workout_preserves_existing_completed_workout_link_over_new_fallback() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-existing",
            "2026-05-10",
        ))
        .await
        .unwrap();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-fallback",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let mut existing = sample_completed_workout();
    existing.planned_workout_id = Some("planned-existing".to_string());
    completed_workouts.upsert(existing).await.unwrap();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-existing".to_string(),
            "completed-imported-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_000,
        ))
        .await
        .unwrap();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-preserve-existing-link".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-existing")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-existing");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Explicit
    );
}

#[tokio::test]
async fn import_completed_workout_does_not_preserve_legacy_planned_id_without_link_row() {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-fallback",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let mut existing = sample_completed_workout();
    existing.planned_workout_id = Some("planned-legacy".to_string());
    completed_workouts.upsert(existing).await.unwrap();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    let service = external_import_service_without_refresh(
        planned_workouts,
        completed_workouts.clone(),
        InMemoryRaceRepository::default(),
        InMemorySpecialDayRepository::default(),
        InMemoryPlannedWorkoutTokenRepository::default(),
        planned_completed_links.clone(),
        InMemoryObservationRepository::default(),
        InMemorySyncStateRepository::default(),
    );

    service
        .import(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
            ExternalCompletedWorkoutImport {
                provider: ExternalProvider::Intervals,
                external_id: "intervals-activity-77".to_string(),
                normalized_payload_hash: "hash-completed-drop-legacy-planned-id".to_string(),
                marker_sources: Vec::new(),
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-fallback")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-fallback");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
}

#[tokio::test]
async fn import_completed_workout_upgrades_existing_heuristic_link_when_token_matches_different_plan(
) {
    let planned_workouts = InMemoryPlannedWorkoutRepository::default();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-heuristic",
            "2026-05-11",
        ))
        .await
        .unwrap();
    planned_workouts
        .upsert(sample_planned_workout_on_date(
            "planned-token",
            "2026-05-11",
        ))
        .await
        .unwrap();
    let completed_workouts = InMemoryCompletedWorkoutRepository::default();
    let mut existing = sample_completed_workout();
    existing.planned_workout_id = Some("planned-heuristic".to_string());
    completed_workouts.upsert(existing).await.unwrap();
    let planned_workout_tokens = InMemoryPlannedWorkoutTokenRepository::default();
    planned_workout_tokens
        .upsert(PlannedWorkoutToken::new(
            "user-1".to_string(),
            "planned-token".to_string(),
            "AB123CDE45".to_string(),
        ))
        .await
        .unwrap();
    let planned_completed_links = InMemoryPlannedCompletedWorkoutLinkRepository::default();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-heuristic".to_string(),
            "completed-imported-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            1_700_000_000,
        ))
        .await
        .unwrap();
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
                normalized_payload_hash: "hash-completed-upgrade-to-token".to_string(),
                marker_sources: vec!["Warmup notes\n[AIWATTCOACH:pw=AB123CDE45]".to_string()],
                workout: sample_completed_workout(),
            },
        )))
        .await
        .unwrap();

    let stored = completed_workouts.list_by_user_id("user-1").await.unwrap();
    assert_eq!(
        stored[0].planned_workout_id.as_deref(),
        Some("planned-token")
    );

    let link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-imported-1")
        .await
        .unwrap()
        .expect("planned-completed link");
    assert_eq!(link.planned_workout_id, "planned-token");
    assert_eq!(
        link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Token
    );
}
