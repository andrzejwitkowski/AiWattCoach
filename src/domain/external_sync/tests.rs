use super::{
    CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalObjectKind,
    ExternalObservation, ExternalObservationParams, ExternalProvider, ExternalSyncState,
    ExternalSyncStatus, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
};

#[test]
fn external_observation_can_point_to_local_canonical_entity() {
    let observation = ExternalObservation::new(ExternalObservationParams {
        user_id: "user-1".to_string(),
        provider: ExternalProvider::Intervals,
        external_object_kind: ExternalObjectKind::CompletedWorkout,
        external_id: "remote-1".to_string(),
        canonical_entity: CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "completed-1".to_string(),
        ),
        normalized_payload_hash: Some("hash-1".to_string()),
        dedup_key: Some("dedup-1".to_string()),
        observed_at_epoch_seconds: 1_700_000_000,
    });

    assert_eq!(observation.user_id, "user-1");
    assert_eq!(observation.provider, ExternalProvider::Intervals);
    assert_eq!(observation.external_id, "remote-1");
    assert_eq!(observation.canonical_entity.entity_id, "completed-1");
}

#[test]
fn sync_state_roundtrip_echo_is_not_a_conflict() {
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::PlannedWorkout, "planned-1".to_string()),
    )
    .record_local_push("hash-1".to_string(), 1_700_000_000)
    .observe_remote("hash-1".to_string(), 1_700_000_100);

    assert_eq!(state.last_synced_payload_hash.as_deref(), Some("hash-1"));
    assert_eq!(
        state.last_seen_remote_payload_hash.as_deref(),
        Some("hash-1")
    );
    assert_eq!(state.conflict_status, ConflictStatus::InSync);
}

#[test]
fn sync_state_divergence_after_sync_marks_conflict() {
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::PlannedWorkout, "planned-1".to_string()),
    )
    .record_local_push("hash-1".to_string(), 1_700_000_000)
    .observe_remote("hash-2".to_string(), 1_700_000_100);

    assert_eq!(state.conflict_status, ConflictStatus::ConflictDetected);
}

#[test]
fn sync_state_mark_synced_sets_provider_link_and_status() {
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced("77".to_string(), "hash-1".to_string(), 1_700_000_000);

    assert_eq!(state.external_id.as_deref(), Some("77"));
    assert_eq!(state.sync_status, ExternalSyncStatus::Synced);
    assert_eq!(state.last_error, None);
}

#[test]
fn sync_state_mark_failed_preserves_external_link() {
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced("77".to_string(), "hash-1".to_string(), 1_700_000_000)
    .mark_failed("boom".to_string());

    assert_eq!(state.external_id.as_deref(), Some("77"));
    assert_eq!(state.sync_status, ExternalSyncStatus::Failed);
    assert_eq!(state.last_error.as_deref(), Some("boom"));
}

#[test]
fn sync_state_mark_failed_clears_stale_conflict_status() {
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .record_local_push("hash-1".to_string(), 1_700_000_000)
    .observe_remote("hash-2".to_string(), 1_700_000_100)
    .mark_failed("boom".to_string());

    assert_eq!(state.sync_status, ExternalSyncStatus::Failed);
    assert_eq!(state.conflict_status, ConflictStatus::Unknown);
}

fn assert_provider_poll_repository<T: ProviderPollStateRepository>() {}

#[test]
fn provider_poll_state_repository_trait_is_usable() {
    assert_provider_poll_repository::<super::ports::NoopProviderPollStateRepository>();
}

#[test]
fn provider_poll_state_reports_when_due() {
    let state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_700_000_000,
    );

    assert!(state.is_due(1_700_000_000));
    assert!(!state.is_due(1_699_999_999));
}

#[test]
fn provider_poll_state_backoff_blocks_due_until_window_expires() {
    let state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_700_000_000,
    )
    .mark_failed(
        "temporary error".to_string(),
        1_700_000_010,
        1_700_000_020,
        Some(1_700_000_120),
    );

    assert!(!state.is_due(1_700_000_100));
    assert!(state.is_due(1_700_000_120));
}

#[test]
fn provider_poll_state_mark_succeeded_updates_cursor_and_clears_error() {
    let state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::CompletedWorkouts,
        1_700_000_000,
    )
    .mark_attempted(1_700_000_010)
    .mark_failed(
        "temporary error".to_string(),
        1_700_000_010,
        1_700_000_020,
        Some(1_700_000_120),
    )
    .mark_succeeded(Some("cursor-2".to_string()), 1_700_000_200, 1_700_000_500);

    assert_eq!(state.cursor.as_deref(), Some("cursor-2"));
    assert_eq!(state.last_error, None);
    assert_eq!(state.last_successful_at_epoch_seconds, Some(1_700_000_200));
    assert_eq!(state.backoff_until_epoch_seconds, None);
    assert_eq!(state.next_due_at_epoch_seconds, 1_700_000_500);
}
