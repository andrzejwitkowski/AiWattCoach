use super::{
    CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalObjectKind,
    ExternalObservation, ExternalProvider, ExternalSyncState, ExternalSyncStatus,
    ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
};

#[test]
fn external_observation_can_point_to_local_canonical_entity() {
    let observation = ExternalObservation::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ExternalObjectKind::CompletedWorkout,
        "remote-1".to_string(),
        CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "completed-1".to_string(),
        ),
        Some("hash-1".to_string()),
        1_700_000_000,
    );

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
