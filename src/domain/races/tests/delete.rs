use super::super::service::RaceService;
use super::support::*;
use crate::domain::{
    calendar_view::project_race_entry,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository, ExternalSyncStatus,
    },
    races::{
        imported_intervals_race_id, Race, RaceDiscipline, RaceError, RacePriority, RaceUseCases,
    },
};

#[tokio::test]
async fn delete_race_deletes_remote_event_before_local_remove() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    sync_states
        .upsert(
            ExternalSyncState::new("user-1".to_string(), ExternalProvider::Intervals, race_ref)
                .mark_synced("88".to_string(), "hash".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(refresh.clone());

    service.delete_race("user-1", "race-1").await.unwrap();

    assert!(repository.stored().is_empty());
    assert_eq!(*intervals.deleted_event_ids.lock().unwrap(), vec![88]);
    assert!(sync_states.stored().is_empty());
    assert_eq!(
        refresh.stored(),
        vec![(
            "user-1".to_string(),
            "2026-09-12".to_string(),
            "2026-09-12".to_string()
        )]
    );
}

#[tokio::test]
async fn delete_race_keeps_sync_state_when_local_delete_fails() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing])
        .with_delete_error(RaceError::Internal("db boom".to_string()));
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Intervals,
                race_ref.clone(),
            )
            .mark_synced("88".to_string(), "hash".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(refresh.clone());

    let error = service.delete_race("user-1", "race-1").await.unwrap_err();

    assert_eq!(error, RaceError::Internal("db boom".to_string()));
    assert_eq!(*intervals.deleted_event_ids.lock().unwrap(), vec![88]);
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(
        stored_states[0].sync_status,
        ExternalSyncStatus::PendingDelete
    );
    assert_eq!(stored_states[0].external_id.as_deref(), Some("88"));
    assert_eq!(stored_states[0].canonical_entity, race_ref);
    assert_eq!(
        refresh.stored(),
        vec![(
            "user-1".to_string(),
            "2026-09-12".to_string(),
            "2026-09-12".to_string()
        )]
    );
}

#[tokio::test]
async fn delete_race_refreshes_calendar_view_when_sync_state_delete_fails_after_local_delete() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let sync_states = InMemoryExternalSyncStateRepository::with_delete_error(
        ExternalSyncRepositoryError::Storage("sync delete boom".to_string()),
    );
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    sync_states
        .upsert(
            ExternalSyncState::new("user-1".to_string(), ExternalProvider::Intervals, race_ref)
                .mark_synced("88".to_string(), "hash".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states,
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(refresh.clone());

    service.delete_race("user-1", "race-1").await.unwrap();

    assert!(repository.stored().is_empty());
    assert_eq!(*intervals.deleted_event_ids.lock().unwrap(), vec![88]);
    assert_eq!(
        refresh.stored(),
        vec![(
            "user-1".to_string(),
            "2026-09-12".to_string(),
            "2026-09-12".to_string()
        )]
    );
}

#[tokio::test]
async fn delete_race_fails_when_calendar_refresh_fails_after_local_delete() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let service = RaceService::new(
        repository.clone(),
        RecordingIntervalsService::default(),
        InMemoryExternalSyncStateRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(FailingCalendarRefresh);

    let error = service.delete_race("user-1", "race-1").await.unwrap_err();

    assert!(repository.stored().is_empty());
    assert!(matches!(error, RaceError::Internal(message) if message.contains("refresh boom")));
}

#[tokio::test]
async fn delete_race_clears_calendar_race_entries_via_refresh() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Delete Me".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let refresh = ClearingCalendarRefresh::with_views(vec![project_race_entry(&existing, None)]);
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let service = RaceService::new(
        repository.clone(),
        RecordingIntervalsService::default(),
        InMemoryExternalSyncStateRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(refresh.clone());

    assert_eq!(refresh.race_entry_ids(), vec!["race:race-1".to_string()]);

    service.delete_race("user-1", "race-1").await.unwrap();

    assert!(repository.stored().is_empty());
    assert!(refresh.race_entry_ids().is_empty());
}

#[tokio::test]
async fn delete_race_removes_intervals_imported_twin_for_linked_event() {
    let local = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Local Race".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let twin = Race {
        race_id: imported_intervals_race_id(88),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Imported Twin".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let twin_entry = project_race_entry(&twin, None);
    let repository = InMemoryRaceRepository::with_races(vec![local.clone(), twin]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let local_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    let twin_ref =
        CanonicalEntityRef::new(CanonicalEntityKind::Race, imported_intervals_race_id(88));
    sync_states
        .upsert(
            ExternalSyncState::new("user-1".to_string(), ExternalProvider::Intervals, local_ref)
                .mark_synced("88".to_string(), "hash-local".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Intervals,
                twin_ref.clone(),
            )
            .mark_synced("88".to_string(), "hash-twin".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let refresh =
        ClearingCalendarRefresh::with_views(vec![project_race_entry(&local, None), twin_entry]);
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(refresh.clone());

    service.delete_race("user-1", "race-1").await.unwrap();

    assert!(repository.stored().is_empty());
    assert_eq!(*intervals.deleted_event_ids.lock().unwrap(), vec![88]);
    assert!(sync_states.stored().is_empty());
    assert!(refresh.race_entry_ids().is_empty());
}
