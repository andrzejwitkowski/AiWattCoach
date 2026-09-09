#![allow(unused_imports)]
use crate::domain::{
    completed_workouts::{
        AuthoritativeCompletedWorkoutRepository, CompletedWorkout, CompletedWorkoutDetails,
        CompletedWorkoutMetrics, CompletedWorkoutRepository, CompletedWorkoutSeries,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository,
    },
    identity::Clock,
    planned_completed_links::{
        PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
        PlannedCompletedWorkoutLinkRepository,
    },
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutStep,
        PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    races::{Race, RaceDiscipline, RacePriority, RaceRepository},
    special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::super::ports::InMemoryCalendarEntryViewRepository;
use super::super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, verify_calendar_entry_integrity, CalendarEntryIntegrityIssue,
    CalendarEntryKind, CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService,
    CalendarEntryViewRepository, CalendarEntryViewService, CalendarPlannedSyncKey,
    CalendarPlannedWorkoutCandidate, CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
    ManualCalendarRefreshService, ManualCalendarRefreshUseCases,
};
use super::support::*;

#[tokio::test]
async fn calendar_entry_view_service_lists_mixed_entries_by_date_range() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository.clone());

    service
        .upsert_planned_workout(&sample_planned_workout(), &[])
        .await
        .unwrap();
    service
        .upsert_completed_workout(&sample_completed_workout())
        .await
        .unwrap();
    service
        .upsert_race(&sample_race(), Some(&sample_race_sync_state()))
        .await
        .unwrap();
    service
        .upsert_special_day(&sample_special_day())
        .await
        .unwrap();

    let entries = service
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(entries.len(), 4);
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::PlannedWorkout));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::CompletedWorkout));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::Race));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::SpecialDay));
}

#[tokio::test]
async fn rebuild_for_user_replaces_stale_entries_and_stays_idempotent() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository.clone());

    repository
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let rebuilt_once = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();
    let rebuilt_twice = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();

    assert_eq!(rebuilt_once, rebuilt_twice);

    let persisted = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(persisted.len(), 3);
    let planned = persisted
        .iter()
        .find(|entry| entry.entry_id == "planned:planned-1")
        .expect("planned entry should remain");
    assert_eq!(planned.completed_workout_id.as_deref(), Some("completed-1"));
    assert_eq!(
        planned
            .summary
            .as_ref()
            .and_then(|summary| summary.training_stress_score),
        Some(82)
    );
    assert!(!persisted
        .iter()
        .any(|entry| entry.entry_id == "completed:completed-1"));
    assert!(persisted
        .iter()
        .all(|entry| entry.entry_id != "special:special-stale"));
}

#[tokio::test]
async fn rebuild_for_user_preserves_existing_sync_metadata() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository.clone()).with_sync_states(
        TestExternalSyncStateRepository::with_states(vec![
            sample_planned_sync_state(),
            sample_race_sync_state(),
        ]),
    );

    repository
        .upsert(project_planned_workout_entry(
            &sample_planned_workout(),
            std::slice::from_ref(&sample_planned_sync_state()),
        ))
        .await
        .unwrap();
    repository
        .upsert(project_race_entry(
            &sample_race(),
            Some(&sample_race_sync_state()),
        ))
        .await
        .unwrap();

    let rebuilt = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();

    let planned = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "planned:planned-1")
        .expect("planned entry after rebuild");
    let race = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "race:race-1")
        .expect("race entry after rebuild");

    assert_eq!(
        planned
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(77)
    );
    assert_eq!(
        planned
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("modified")
    );
    assert_eq!(
        race.sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(41)
    );
}

#[tokio::test]
async fn rebuild_for_user_uses_authoritative_sync_when_view_store_is_empty() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![
        ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(
                CanonicalEntityKind::PlannedWorkout,
                "plan-op-1:2026-05-10".to_string(),
            ),
        )
        .mark_synced("88".to_string(), "hash-1".to_string(), 2),
        ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
        )
        .mark_synced("42".to_string(), "hash-2".to_string(), 3),
    ]);
    let service = CalendarEntryViewService::new(repository).with_sync_states(sync_states);

    let rebuilt = service
        .rebuild_for_user(
            "user-1",
            &[sample_bridged_planned_workout("plan-op-1", "2026-05-10")],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();

    let planned = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "planned:plan-op-1:2026-05-10")
        .expect("planned entry after authoritative rebuild");
    let race = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "race:race-1")
        .expect("race entry after authoritative rebuild");

    assert_eq!(
        planned
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(88)
    );
    assert_eq!(
        race.sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(42)
    );
}

#[tokio::test]
async fn rebuild_for_user_uses_external_sync_state_for_imported_planned_workouts() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "imported-planned-1".to_string(),
        ),
    )
    .mark_synced("144".to_string(), "hash-1".to_string(), 2)]);
    let service = CalendarEntryViewService::new(repository).with_sync_states(sync_states);

    let rebuilt = service
        .rebuild_for_user(
            "user-1",
            &[PlannedWorkout::new(
                "imported-planned-1".to_string(),
                "user-1".to_string(),
                "2026-05-10".to_string(),
                sample_planned_workout().workout,
            )],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();

    let planned = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "planned:imported-planned-1")
        .expect("imported planned entry after rebuild");

    assert_eq!(
        planned
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(144)
    );
    assert_eq!(
        planned
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("modified")
    );
}

#[tokio::test]
async fn rebuild_for_user_keeps_sync_on_merged_planned_entry_without_standalone_completed_entry() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository)
        .with_sync_states(TestExternalSyncStateRepository::default());

    let rebuilt = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[],
            &[],
        )
        .await
        .unwrap();

    let planned = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "planned:planned-1")
        .expect("planned entry after rebuild");

    assert_eq!(planned.sync, None);
    assert_eq!(planned.completed_workout_id.as_deref(), Some("completed-1"));
    assert!(!rebuilt
        .iter()
        .any(|entry| entry.entry_id == "completed:completed-1"));
}

#[tokio::test]
async fn rebuild_for_user_clears_stale_planned_sync_when_external_state_is_missing() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let mut stale_entry = project_planned_workout_entry(&sample_planned_workout(), &[]);
    stale_entry.sync = Some(super::super::CalendarEntrySync {
        linked_intervals_event_id: Some(77),
        sync_status: Some("synced".to_string()),
    });
    repository.upsert(stale_entry).await.unwrap();

    let service = CalendarEntryViewService::new(repository)
        .with_sync_states(TestExternalSyncStateRepository::default());

    let rebuilt = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[],
            &[],
        )
        .await
        .unwrap();

    let planned = rebuilt
        .iter()
        .find(|entry| entry.entry_id == "planned:planned-1")
        .expect("planned entry after rebuild");

    assert_eq!(planned.sync, None);
}

#[tokio::test]
async fn replace_range_for_user_replaces_only_target_range_and_handles_date_moves() {
    let repository = InMemoryCalendarEntryViewRepository::default();

    repository
        .upsert(project_planned_workout_entry(
            &sample_planned_workout(),
            &[],
        ))
        .await
        .unwrap();
    repository
        .upsert(project_race_entry(&sample_race(), None))
        .await
        .unwrap();
    repository
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let mut moved_planned = sample_planned_workout();
    moved_planned.date = "2026-05-15".to_string();

    repository
        .replace_range_for_user(
            "user-1",
            "2026-05-10",
            "2026-05-12",
            vec![project_planned_workout_entry(&moved_planned, &[])],
        )
        .await
        .unwrap();

    let entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry.entry_id == "planned:planned-1" && entry.date == "2026-05-15"));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_id == "special:special-stale"));
    assert!(!entries.iter().any(|entry| entry.entry_id == "race:race-1"));
}

#[tokio::test]
async fn replace_all_for_user_rejects_mismatched_user_entries() {
    let repository = InMemoryCalendarEntryViewRepository::default();

    let error = repository
        .replace_all_for_user(
            "user-1",
            vec![project_special_day_entry(&sample_special_day_for_user(
                "user-2",
            ))],
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        super::super::CalendarEntryViewError::Repository(
            "calendar entry user mismatch for replace_all_for_user: expected user-1, got user-2"
                .to_string()
        )
    );
}

#[tokio::test]
async fn replace_range_for_user_rejects_mismatched_user_entries() {
    let repository = InMemoryCalendarEntryViewRepository::default();

    let error = repository
        .replace_range_for_user(
            "user-1",
            "2026-05-10",
            "2026-05-10",
            vec![project_special_day_entry(&sample_special_day_for_user(
                "user-2",
            ))],
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        super::super::CalendarEntryViewError::Repository(
            "calendar entry user mismatch for replace_range_for_user: expected user-1, got user-2"
                .to_string()
        )
    );
}
