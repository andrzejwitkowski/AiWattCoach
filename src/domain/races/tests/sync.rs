use super::super::service::RaceService;
use super::support::*;
use crate::domain::{
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository, ExternalSyncStatus, ProviderPollStream,
    },
    intervals::{Event, EventCategory},
    races::{CreateRace, Race, RaceDiscipline, RaceError, RacePriority, RaceUseCases, UpdateRace},
};

#[tokio::test]
async fn create_race_persists_and_syncs_to_intervals() {
    let repository = InMemoryRaceRepository::default();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let poll_states = InMemoryProviderPollStateRepository::default();
    let intervals = RecordingIntervalsService::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        repository.clone(),
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_provider_poll_states(poll_states.clone())
    .with_calendar_view_refresh(refresh.clone());

    let created = service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-12".to_string(),
                name: "Gravel Attack".to_string(),
                distance_meters: 120_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::B,
            },
        )
        .await
        .unwrap();

    assert_eq!(created.race_id, "race-123");
    let created_events = intervals.created_events.lock().unwrap();
    assert_eq!(created_events.len(), 1);
    assert_eq!(created_events[0].category, EventCategory::RaceB);
    assert_eq!(
        created_events[0].description.as_deref(),
        Some("distance_meters=120000\ndiscipline=gravel\npriority=B\ncanonical_race_id=race-123")
    );
    assert_eq!(repository.stored().len(), 1);
    let sync_state = sync_states.stored().pop().expect("expected sync state");
    assert_eq!(sync_state.external_id.as_deref(), Some("77"));
    assert_eq!(sync_state.sync_status, ExternalSyncStatus::Synced);
    let poll_state = poll_states.stored().pop().expect("expected poll state");
    assert_eq!(poll_state.stream, ProviderPollStream::Calendar);
    assert_eq!(poll_state.next_due_at_epoch_seconds, 1_700_000_000);
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
async fn priority_race_creates_matching_intervals_categories() {
    let repository = InMemoryRaceRepository::default();
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states,
        TestClock,
        TestIdGenerator::default(),
    );

    service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-12".to_string(),
                name: "Peak A Race".to_string(),
                distance_meters: 140_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap();

    service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-19".to_string(),
                name: "Support C Race".to_string(),
                distance_meters: 80_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::C,
            },
        )
        .await
        .unwrap();

    let created_events = intervals.created_events.lock().unwrap();
    assert_eq!(created_events.len(), 2);
    assert_eq!(created_events[0].category, EventCategory::RaceA);
    assert_eq!(created_events[1].category, EventCategory::RaceC);
}

#[tokio::test]
async fn update_race_marks_failure_when_intervals_update_fails() {
    let existing = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Old Name".to_string(),
        distance_meters: 100_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::C,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let repository = InMemoryRaceRepository::with_races(vec![existing]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let poll_states = InMemoryProviderPollStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string());
    let existing_sync = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        race_ref.clone(),
    )
    .mark_synced("55".to_string(), "old-hash".to_string(), 2);
    sync_states
        .upsert(existing_sync)
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::with_failed_updates();
    let service = RaceService::new(
        repository.clone(),
        intervals,
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_provider_poll_states(poll_states.clone());

    let error = service
        .update_race(
            "user-1",
            "race-1",
            UpdateRace {
                date: "2026-09-13".to_string(),
                name: "New Name".to_string(),
                distance_meters: 130_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, RaceError::Unavailable("boom".to_string()));
    let stored = repository.stored();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].name, "New Name");
    let updated_sync = sync_states.stored().pop().expect("expected sync state");
    assert_eq!(updated_sync.sync_status, ExternalSyncStatus::Failed);
    assert_eq!(updated_sync.last_error.as_deref(), Some("boom"));
    assert_eq!(updated_sync.canonical_entity, race_ref);
    assert!(poll_states.stored().is_empty());
}

#[tokio::test]
async fn create_race_retry_after_lost_final_sync_write_updates_existing_remote_event() {
    let repository = InMemoryRaceRepository::default();
    let sync_states = InMemoryExternalSyncStateRepository::with_dropped_synced_writes();
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states,
        TestClock,
        TestIdGenerator::default(),
    );

    let created = service
        .create_race(
            "user-1",
            CreateRace {
                date: "2026-09-12".to_string(),
                name: "Gravel Attack".to_string(),
                distance_meters: 120_000,
                discipline: RaceDiscipline::Gravel,
                priority: RacePriority::B,
            },
        )
        .await
        .unwrap();

    service
        .update_race(
            "user-1",
            &created.race_id,
            UpdateRace {
                date: "2026-09-13".to_string(),
                name: "Updated Attack".to_string(),
                distance_meters: 121_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap();

    assert_eq!(intervals.created_events.lock().unwrap().len(), 1);
    assert_eq!(intervals.updated_events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn create_race_retry_without_external_id_reuses_existing_remote_event() {
    let repository = InMemoryRaceRepository::with_races(vec![Race {
        race_id: "race-123".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let intervals = RecordingIntervalsService::with_listed_events(vec![Event {
        id: 77,
        start_date_local: "2026-09-12T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Race Gravel Attack".to_string()),
        category: EventCategory::RaceB,
        description: Some(
            "distance_meters=120000\ndiscipline=gravel\npriority=B\ncanonical_race_id=race-123"
                .to_string(),
        ),
        indoor: false,
        color: None,
        workout_doc: None,
    }]);
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    );

    service
        .update_race(
            "user-1",
            "race-123",
            UpdateRace {
                date: "2026-09-12".to_string(),
                name: "Updated Gravel Attack".to_string(),
                distance_meters: 125_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap();

    assert_eq!(intervals.created_events.lock().unwrap().len(), 0);
    assert_eq!(intervals.updated_events.lock().unwrap().len(), 1);
    let updated_events = intervals.updated_events.lock().unwrap();
    assert_eq!(updated_events[0].0, 77);
    assert_eq!(
        updated_events[0].1.name.as_deref(),
        Some("Race Updated Gravel Attack")
    );
    assert_eq!(updated_events[0].1.category, Some(EventCategory::RaceA));
    assert_eq!(
        updated_events[0].1.description.as_deref(),
        Some("distance_meters=125000\ndiscipline=road\npriority=A\ncanonical_race_id=race-123")
    );
    let stored_sync = sync_states.stored();
    assert_eq!(stored_sync.len(), 1);
    assert_eq!(stored_sync[0].external_id.as_deref(), Some("77"));
}

#[tokio::test]
async fn update_race_ignores_other_users_sync_state_and_calls_intervals_with_current_user() {
    let repository = InMemoryRaceRepository::with_races(vec![Race {
        race_id: "race-123".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let race_ref = CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-123".to_string());
    sync_states
        .upsert(
            ExternalSyncState::new("user-2".to_string(), ExternalProvider::Intervals, race_ref)
                .mark_synced("88".to_string(), "hash".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let intervals = RecordingIntervalsService::default();
    let service = RaceService::new(
        repository,
        intervals.clone(),
        sync_states.clone(),
        TestClock,
        TestIdGenerator::default(),
    );

    service
        .update_race(
            "user-1",
            "race-123",
            UpdateRace {
                date: "2026-09-12".to_string(),
                name: "Updated Gravel Attack".to_string(),
                distance_meters: 125_000,
                discipline: RaceDiscipline::Road,
                priority: RacePriority::A,
            },
        )
        .await
        .unwrap();

    assert_eq!(intervals.created_events.lock().unwrap().len(), 1);
    assert_eq!(intervals.updated_events.lock().unwrap().len(), 0);
    assert_eq!(
        *intervals.list_event_user_ids.lock().unwrap(),
        vec!["user-1".to_string()]
    );
    assert_eq!(
        *intervals.create_event_user_ids.lock().unwrap(),
        vec!["user-1".to_string()]
    );
    assert!(intervals.update_event_user_ids.lock().unwrap().is_empty());

    let stored_sync = sync_states.stored();
    assert_eq!(stored_sync.len(), 2);
    assert!(stored_sync
        .iter()
        .any(|state| { state.user_id == "user-2" && state.external_id.as_deref() == Some("88") }));
    assert!(stored_sync.iter().any(|state| {
        state.user_id == "user-1"
            && state.external_id.as_deref() == Some("77")
            && state.sync_status == ExternalSyncStatus::Synced
    }));
}
