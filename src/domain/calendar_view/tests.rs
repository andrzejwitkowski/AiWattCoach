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

use super::ports::InMemoryCalendarEntryViewRepository;
use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, verify_calendar_entry_integrity, CalendarEntryIntegrityIssue,
    CalendarEntryKind, CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService,
    CalendarEntryViewRepository, CalendarEntryViewService, CalendarPlannedSyncKey,
    CalendarPlannedWorkoutCandidate, CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
    ManualCalendarRefreshService, ManualCalendarRefreshUseCases,
};

#[derive(Clone, Copy)]
struct FixedClock(i64);

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.0
    }
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> super::BoxFuture<Result<Vec<super::CalendarEntryView>, super::CalendarEntryViewError>>
    {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest, newest.clone()));
            Ok(vec![sample_calendar_entry_with_date(&newest)])
        })
    }
}

#[tokio::test]
async fn manual_calendar_refresh_uses_oldest_date_across_sources_and_existing_view() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let refresh = RecordingCalendarRefresh::default();

    views
        .upsert(sample_calendar_entry_with_date("2026-04-15"))
        .await
        .unwrap();
    planned.upsert(
        sample_planned_workout(),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );
    let mut completed_workout = sample_completed_workout();
    completed_workout.start_date_local = "2026-05-09T08:00:00".to_string();
    completed.upsert(completed_workout).await.unwrap();
    races.upsert(sample_race()).await.unwrap();
    special_days.upsert(sample_special_day()).await.unwrap();

    let service = ManualCalendarRefreshService::new(
        views,
        planned,
        completed,
        races,
        special_days,
        FixedClock(1_777_248_000),
        refresh.clone(),
    );

    let result = service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(result.oldest, "2026-04-15");
    assert_eq!(result.newest, "2026-05-13");
    assert_eq!(result.rebuilt_entry_count, 1);
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-04-15".to_string(),
            "2026-05-13".to_string(),
        )]
    );
}

#[tokio::test]
async fn manual_calendar_refresh_falls_back_to_today_when_user_has_no_calendar_sources() {
    let service = ManualCalendarRefreshService::new(
        InMemoryCalendarEntryViewRepository::default(),
        TestCalendarPlannedWorkoutSource::default(),
        TestCompletedWorkoutRepository::default(),
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        FixedClock(1_777_248_000),
        RecordingCalendarRefresh::default(),
    );

    let result = service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(result.oldest, "2026-04-27");
    assert_eq!(result.newest, "2026-04-27");
}

#[tokio::test]
async fn manual_calendar_refresh_skips_malformed_completed_workout_dates() {
    let completed = TestCompletedWorkoutRepository::default();
    let mut malformed = sample_completed_workout();
    malformed.start_date_local = "bad-date".to_string();
    completed.upsert(malformed).await.unwrap();

    let service = ManualCalendarRefreshService::new(
        InMemoryCalendarEntryViewRepository::default(),
        TestCalendarPlannedWorkoutSource::default(),
        completed,
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        FixedClock(1_777_248_000),
        RecordingCalendarRefresh::default(),
    );

    let result = service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(result.oldest, "2026-04-27");
    assert_eq!(result.newest, "2026-04-27");
}

#[tokio::test]
async fn manual_calendar_refresh_extends_newest_for_future_only_calendar_data() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let refresh = RecordingCalendarRefresh::default();

    let mut future_workout = sample_planned_workout();
    future_workout.date = "2026-06-02".to_string();
    future_workout.planned_workout_id = "planned-future".to_string();
    planned.upsert(
        future_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );
    views
        .upsert(sample_calendar_entry_with_date("2026-06-03"))
        .await
        .unwrap();

    let service = ManualCalendarRefreshService::new(
        views,
        planned,
        TestCompletedWorkoutRepository::default(),
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        FixedClock(1_777_248_000),
        refresh.clone(),
    );

    let result = service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(result.oldest, "2026-06-02");
    assert_eq!(result.newest, "2026-06-03");
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-06-02".to_string(),
            "2026-06-03".to_string(),
        )]
    );
}

#[tokio::test]
async fn manual_calendar_refresh_preserves_future_only_existing_view_range() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let refresh = RecordingCalendarRefresh::default();

    views
        .upsert(sample_calendar_entry_with_date("2026-06-05"))
        .await
        .unwrap();

    let service = ManualCalendarRefreshService::new(
        views,
        TestCalendarPlannedWorkoutSource::default(),
        TestCompletedWorkoutRepository::default(),
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        FixedClock(1_777_248_000),
        refresh.clone(),
    );

    let result = service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    assert_eq!(result.oldest, "2026-06-05");
    assert_eq!(result.newest, "2026-06-05");
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-06-05".to_string(),
            "2026-06-05".to_string(),
        )]
    );
}

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

#[test]
fn planned_workout_projection_marks_entry_modified_when_payload_hash_changed() {
    let entry =
        project_planned_workout_entry(&sample_planned_workout(), &[sample_planned_sync_state()]);

    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(77)
    );
    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("modified")
    );
}

#[test]
fn planned_workout_projection_prefers_failed_status_over_modified_hash() {
    let state = sample_planned_sync_state().mark_failed("boom".to_string());

    let entry = project_planned_workout_entry(&sample_planned_workout(), &[state]);

    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("failed")
    );
}

#[test]
fn planned_workout_projection_stays_synced_when_stored_hash_matches_intervals_sync_hash() {
    let workout = sample_bridged_planned_workout("plan-op-1", "2026-05-10");
    let parsed = crate::domain::planned_workouts::to_intervals_planned_workout(&workout).unwrap();
    let payload_hash = crate::domain::planned_workouts::intervals_planned_workout_payload_hash(
        &workout.date,
        &parsed,
        Some("Threshold builder"),
    );
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "plan-op-1:2026-05-10".to_string(),
        ),
    )
    .mark_synced("77".to_string(), payload_hash, 1_700_000_000);

    let entry = project_planned_workout_entry(&workout, &[state]);

    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("synced")
    );
}

#[test]
fn planned_workout_projection_marks_synced_when_hash_matches_update_service_hash() {
    let workout = sample_planned_workout();
    let payload_hash = crate::domain::planned_workouts::planned_workout_payload_hash(&workout);
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::PlannedWorkout, "planned-1".to_string()),
    )
    .mark_synced("77".to_string(), payload_hash, 1_700_000_000);

    let entry = project_planned_workout_entry(&workout, &[state]);

    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("synced")
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
    stale_entry.sync = Some(super::CalendarEntrySync {
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
        super::CalendarEntryViewError::Repository(
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
        super::CalendarEntryViewError::Repository(
            "calendar entry user mismatch for replace_range_for_user: expected user-1, got user-2"
                .to_string()
        )
    );
}

#[tokio::test]
async fn refresh_range_for_user_rebuilds_only_requested_dates() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();

    planned.upsert(
        sample_planned_workout(),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );
    completed.upsert(sample_completed_workout()).await.unwrap();
    races.upsert(sample_race()).await.unwrap();
    special_days.upsert(sample_special_day()).await.unwrap();
    views
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views.clone(),
        planned,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-13")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 3);
    let planned = refreshed
        .iter()
        .find(|entry| entry.entry_id == "planned:planned-1")
        .expect("planned entry after refresh");
    assert_eq!(planned.completed_workout_id.as_deref(), Some("completed-1"));
    assert_eq!(
        planned
            .summary
            .as_ref()
            .and_then(|summary| summary.training_stress_score),
        Some(82)
    );
    assert!(!refreshed
        .iter()
        .any(|entry| entry.entry_id == "completed:completed-1"));

    let all_entries = views
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(all_entries.len(), 4);
    assert!(all_entries
        .iter()
        .any(|entry| entry.entry_id == "special:special-stale"));
}

#[tokio::test]
async fn refresh_range_for_user_uses_external_sync_states_for_planned_entries() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "plan-op-1:2026-05-10".to_string(),
        ),
    )
    .mark_synced("55".to_string(), "hash-1".to_string(), 2)]);

    planned.upsert(
        sample_bridged_planned_workout("plan-op-1", "2026-05-10"),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views.clone(),
        planned,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(55)
    );
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("modified")
    );
}

#[tokio::test]
async fn refresh_range_for_user_uses_external_sync_state_for_imported_planned_workouts() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "imported-planned-1".to_string(),
        ),
    )
    .mark_synced("144".to_string(), "hash-1".to_string(), 2)]);

    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            sample_planned_workout().workout,
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(144)
    );
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("modified")
    );
}

#[tokio::test]
async fn refresh_range_for_user_prefers_imported_planned_workout_override_over_projected_candidate()
{
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "imported-planned-1".to_string(),
        ),
    )
    .mark_synced("144".to_string(), "hash-1".to_string(), 2);
    let sync_key = CalendarPlannedSyncKey {
        provider: "intervals".to_string(),
        external_id: "144".to_string(),
    };
    let sync_states = TestExternalSyncStateRepository::with_states(vec![sync_state]);

    planned.upsert(
        sample_bridged_planned_workout("plan-op-1", "2026-05-10"),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![sync_key.clone()],
    );
    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            PlannedWorkoutContent {
                lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "AI override".to_string(),
                })],
            },
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![sync_key],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].title, "AI override");
}

#[tokio::test]
async fn refresh_range_for_user_batches_planned_workout_sync_state_lookups() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![
        ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(
                CanonicalEntityKind::PlannedWorkout,
                "imported-planned-1".to_string(),
            ),
        )
        .mark_synced("144".to_string(), "hash-1".to_string(), 2),
        ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(
                CanonicalEntityKind::PlannedWorkout,
                "imported-planned-2".to_string(),
            ),
        )
        .mark_synced("145".to_string(), "hash-2".to_string(), 3),
    ]);

    let sample_workout = sample_planned_workout().workout;
    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            sample_workout.clone(),
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![],
    );
    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-2".to_string(),
            "user-1".to_string(),
            "2026-05-11".to_string(),
            sample_workout,
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed,
        races,
        special_days,
        sync_states.clone(),
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-11")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 2);
    let (single_lookups, batch_lookups) = sync_states.lookup_counts();
    assert_eq!(single_lookups, 0);
    assert_eq!(batch_lookups, 1);
}

#[tokio::test]
async fn refresh_range_for_user_keeps_multiple_distinct_planned_workouts_on_same_day() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();

    let mut second_workout = sample_planned_workout();
    second_workout.planned_workout_id = "planned-2".to_string();
    second_workout.name = Some("Evening opener".to_string());

    planned.upsert(
        sample_planned_workout(),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );
    planned.upsert(
        second_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        TestCompletedWorkoutRepository::default(),
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        TestExternalSyncStateRepository::default(),
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 2);
    assert!(refreshed
        .iter()
        .any(|entry| entry.entry_id == "planned:planned-1"));
    assert!(refreshed
        .iter()
        .any(|entry| entry.entry_id == "planned:planned-2"));
}

#[tokio::test]
async fn refresh_range_for_user_prefers_projected_planned_over_imported_duplicate() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();

    let duplicate_sync_key = CalendarPlannedSyncKey {
        provider: "intervals".to_string(),
        external_id: "144".to_string(),
    };
    planned.upsert(
        sample_bridged_planned_workout("plan-op-1", "2026-05-10"),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![duplicate_sync_key.clone()],
    );
    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            sample_planned_workout().workout,
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![duplicate_sync_key],
    );

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        TestCompletedWorkoutRepository::default(),
        TestRaceRepository::default(),
        TestSpecialDayRepository::default(),
        TestExternalSyncStateRepository::default(),
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_id, "planned:plan-op-1:2026-05-10");
}

#[tokio::test]
async fn refresh_range_for_user_clears_hidden_imported_duplicate_planned_id_before_merging() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let duplicate_sync_key = CalendarPlannedSyncKey {
        provider: "intervals".to_string(),
        external_id: "144".to_string(),
    };
    planned.upsert(
        sample_bridged_planned_workout("plan-op-1", "2026-05-10"),
        CalendarPlannedWorkoutOrigin::Projected,
        vec![duplicate_sync_key.clone()],
    );
    planned.upsert(
        PlannedWorkout::new(
            "imported-planned-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            sample_planned_workout().workout,
        ),
        CalendarPlannedWorkoutOrigin::Imported,
        vec![duplicate_sync_key],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    workout.planned_workout_id = Some("imported-planned-1".to_string());
    workout.name = Some("Threshold builder".to_string());
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "imported-planned-1".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            1_700_000_000,
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_id, "planned:plan-op-1:2026-05-10");
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("plan-op-1:2026-05-10")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("hidden imported duplicate link is replaced");
    assert_eq!(stored_link.planned_workout_id, "plan-op-1:2026-05-10");
    assert_eq!(
        stored_link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
}

#[tokio::test]
async fn refresh_range_for_user_clears_orphaned_heuristic_links_and_replaces_stale_planned_entries()
{
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-1".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            1_700_000_000,
        ))
        .await
        .unwrap();
    views
        .upsert(project_planned_workout_entry(
            &sample_planned_workout(),
            &[],
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views.clone(),
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(stored_workout.planned_workout_id, None);

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap();
    assert_eq!(stored_link, None);

    let persisted = views
        .list_by_user_id_and_date_range("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].entry_kind, CalendarEntryKind::CompletedWorkout);
}

#[tokio::test]
async fn refresh_range_for_user_uses_intervals_completed_workout_when_sparse_wahoo_shares_day() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Wahoo,
        CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "wahoo-workout:2".to_string(),
        ),
    )
    .mark_synced("2".to_string(), "hash-1".to_string(), 1_700_000_000)]);

    let mut intervals = sample_completed_workout();
    intervals.completed_workout_id = "intervals-activity:1".to_string();
    intervals.source_activity_id = Some("shared-activity".to_string());
    intervals.planned_workout_id = None;
    intervals.start_date_local = "2026-05-10T08:00:00".to_string();
    intervals.name = Some("Intervals detailed".to_string());
    completed.upsert(intervals).await.unwrap();

    let mut wahoo = sample_completed_basic_workout();
    wahoo.completed_workout_id = "wahoo-workout:2".to_string();
    wahoo.source_activity_id = Some("shared-activity".to_string());
    wahoo.planned_workout_id = None;
    wahoo.start_date_local = "2026-05-10T08:05:00".to_string();
    wahoo.name = Some("Wahoo sparse".to_string());
    completed.upsert(wahoo).await.unwrap();

    // The authoritative repo needs Wahoo sync metadata, but the refresher's own sync reads stay neutral here.
    let authoritative_completed =
        AuthoritativeCompletedWorkoutRepository::new(completed, sync_states);
    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        authoritative_completed,
        races,
        special_days,
        TestExternalSyncStateRepository::default(),
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("intervals-activity:1")
    );
    assert_eq!(refreshed[0].title, "Intervals detailed");
}

#[tokio::test]
async fn refresh_range_for_user_prefers_wahoo_completed_workout_when_wahoo_has_power_details() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::with_states(vec![ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Wahoo,
        CanonicalEntityRef::new(
            CanonicalEntityKind::CompletedWorkout,
            "wahoo-workout:2".to_string(),
        ),
    )
    .mark_synced("2".to_string(), "hash-1".to_string(), 1_700_000_000)]);

    let mut intervals = sample_completed_basic_workout();
    intervals.completed_workout_id = "intervals-activity:1".to_string();
    intervals.source_activity_id = Some("shared-activity".to_string());
    intervals.planned_workout_id = None;
    intervals.start_date_local = "2026-05-10T08:00:00".to_string();
    intervals.name = Some("Intervals basic".to_string());
    completed.upsert(intervals).await.unwrap();

    let mut wahoo = sample_completed_workout();
    wahoo.completed_workout_id = "wahoo-workout:2".to_string();
    wahoo.source_activity_id = Some("shared-activity".to_string());
    wahoo.planned_workout_id = None;
    wahoo.start_date_local = "2026-05-10T08:05:00".to_string();
    wahoo.name = Some("Wahoo detailed".to_string());
    completed.upsert(wahoo).await.unwrap();

    // The authoritative repo needs Wahoo sync metadata, but the refresher's own sync reads stay neutral here.
    let authoritative_completed =
        AuthoritativeCompletedWorkoutRepository::new(completed, sync_states);
    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        authoritative_completed,
        races,
        special_days,
        TestExternalSyncStateRepository::default(),
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("wahoo-workout:2")
    );
    assert_eq!(refreshed[0].title, "Wahoo detailed");
}

#[tokio::test]
async fn refresh_range_for_user_clears_orphaned_explicit_links() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-1".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_000,
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(refreshed[0].planned_workout_id, None);

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(stored_workout.planned_workout_id, None);

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap();
    assert!(stored_link.is_none());
}

#[tokio::test]
async fn refresh_range_for_user_replaces_stale_explicit_link_with_current_same_day_planned_workout()
{
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut planned_workout = sample_planned_workout();
    planned_workout.planned_workout_id = "planned-new".to_string();
    planned_workout.name = None;
    planned.upsert(
        planned_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    workout.planned_workout_id = Some("planned-old".to_string());
    workout.name = Some("Threshold builder".to_string());
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-old".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Explicit,
            1_700_000_000,
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::PlannedWorkout);
    assert_eq!(
        refreshed[0].planned_workout_id.as_deref(),
        Some("planned-new")
    );
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("planned-new")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("stale explicit link is replaced by relinked heuristic link");
    assert_eq!(stored_link.planned_workout_id, "planned-new");
    assert_eq!(
        stored_link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
    assert_eq!(stored_link.matched_at_epoch_seconds, 1_778_414_400);
}

#[tokio::test]
async fn refresh_range_for_user_preserves_heuristic_link_when_planned_workout_exists_outside_range()
{
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut planned_workout = sample_planned_workout();
    planned_workout.date = "2026-05-11".to_string();
    planned.upsert(
        planned_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-1".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            1_700_000_000,
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(
        refreshed[0].planned_workout_id.as_deref(),
        Some("planned-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("planned-1")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("heuristic link remains stored");
    assert_eq!(
        stored_link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
}

#[tokio::test]
async fn refresh_range_for_user_clears_legacy_orphaned_planned_id_without_link_row() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    completed.upsert(workout).await.unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links);

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::CompletedWorkout);
    assert_eq!(refreshed[0].planned_workout_id, None);

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(stored_workout.planned_workout_id, None);
}

#[tokio::test]
async fn refresh_range_for_user_relinks_completed_workout_to_current_same_day_planned_workout() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut planned_workout = sample_planned_workout();
    planned_workout.planned_workout_id = "planned-new".to_string();
    planned_workout.name = None;
    planned.upsert(
        planned_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    workout.planned_workout_id = None;
    workout.name = Some("Threshold builder".to_string());
    completed.upsert(workout).await.unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::PlannedWorkout);
    assert_eq!(
        refreshed[0].planned_workout_id.as_deref(),
        Some("planned-new")
    );
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("planned-new")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("heuristic link is recreated");
    assert_eq!(stored_link.planned_workout_id, "planned-new");
    assert_eq!(
        stored_link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
    assert_eq!(stored_link.matched_at_epoch_seconds, 1_778_414_400);
}

#[tokio::test]
async fn refresh_range_for_user_replaces_stale_heuristic_link_with_current_same_day_planned_workout(
) {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut planned_workout = sample_planned_workout();
    planned_workout.planned_workout_id = "planned-new".to_string();
    planned_workout.name = None;
    planned.upsert(
        planned_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10T08:00:00".to_string();
    workout.planned_workout_id = Some("planned-old".to_string());
    workout.name = Some("Threshold builder".to_string());
    completed.upsert(workout).await.unwrap();
    planned_completed_links
        .upsert(PlannedCompletedWorkoutLink::new(
            "user-1".to_string(),
            "planned-old".to_string(),
            "completed-1".to_string(),
            PlannedCompletedWorkoutLinkMatchSource::Heuristic,
            1_700_000_000,
        ))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::PlannedWorkout);
    assert_eq!(
        refreshed[0].planned_workout_id.as_deref(),
        Some("planned-new")
    );
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("planned-new")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("heuristic link is moved to current plan");
    assert_eq!(stored_link.planned_workout_id, "planned-new");
    assert_eq!(
        stored_link.match_source,
        PlannedCompletedWorkoutLinkMatchSource::Heuristic
    );
    assert_eq!(stored_link.matched_at_epoch_seconds, 1_778_414_400);
}

#[tokio::test]
async fn refresh_range_for_user_relinks_without_creating_heuristic_link_when_completed_date_is_malformed(
) {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository::default();
    let planned_completed_links = TestPlannedCompletedWorkoutLinkRepository::default();

    let mut planned_workout = sample_planned_workout();
    planned_workout.planned_workout_id = "planned-new".to_string();
    planned_workout.name = None;
    planned.upsert(
        planned_workout,
        CalendarPlannedWorkoutOrigin::Projected,
        vec![],
    );

    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05-10 invalid".to_string();
    workout.planned_workout_id = None;
    workout.name = Some("Threshold builder".to_string());
    completed.upsert(workout).await.unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned,
        completed.clone(),
        races,
        special_days,
        sync_states,
    )
    .with_planned_completed_links(planned_completed_links.clone());

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed[0].entry_kind, CalendarEntryKind::PlannedWorkout);
    assert_eq!(
        refreshed[0].planned_workout_id.as_deref(),
        Some("planned-new")
    );
    assert_eq!(
        refreshed[0].completed_workout_id.as_deref(),
        Some("completed-1")
    );

    let stored_workout = completed
        .find_by_user_id_and_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap()
        .expect("completed workout remains stored");
    assert_eq!(
        stored_workout.planned_workout_id.as_deref(),
        Some("planned-new")
    );

    let stored_link = planned_completed_links
        .find_by_completed_workout_id("user-1", "completed-1")
        .await
        .unwrap();
    assert!(stored_link.is_none());
}

#[derive(Clone, Default)]
struct TestCalendarPlannedWorkoutSource {
    stored: std::sync::Arc<std::sync::Mutex<Vec<CalendarPlannedWorkoutCandidate>>>,
}

impl TestCalendarPlannedWorkoutSource {
    fn upsert(
        &self,
        workout: PlannedWorkout,
        origin: CalendarPlannedWorkoutOrigin,
        sync_keys: Vec<CalendarPlannedSyncKey>,
    ) {
        let mut stored = self.stored.lock().unwrap();
        stored.retain(|existing| {
            !(existing.workout.user_id == workout.user_id
                && existing.workout.planned_workout_id == workout.planned_workout_id)
        });
        stored.push(CalendarPlannedWorkoutCandidate {
            workout,
            origin,
            sync_keys,
        });
    }
}

impl CalendarPlannedWorkoutSource for TestCalendarPlannedWorkoutSource {
    fn list_candidates_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> super::BoxFuture<
        Result<
            Vec<CalendarPlannedWorkoutCandidate>,
            crate::domain::planned_workouts::PlannedWorkoutError,
        >,
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
                .filter(|candidate| candidate.workout.user_id == user_id)
                .filter(|candidate| {
                    candidate.workout.date >= oldest && candidate.workout.date <= newest
                })
                .cloned()
                .collect())
        })
    }
}

#[derive(Clone, Default)]
struct TestCompletedWorkoutRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<CompletedWorkout>>>,
}

#[derive(Clone, Default)]
struct TestPlannedCompletedWorkoutLinkRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<PlannedCompletedWorkoutLink>>>,
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> super::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> super::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
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
    ) -> super::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
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
                    let date = workout
                        .start_date_local
                        .get(..10)
                        .unwrap_or(workout.start_date_local.as_str());
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> super::BoxFuture<
        Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
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

impl PlannedCompletedWorkoutLinkRepository for TestPlannedCompletedWorkoutLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Option<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
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
        Result<
            Option<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
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

    fn find_by_planned_workout_ids(
        &self,
        user_id: &str,
        planned_workout_ids: &[String],
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Vec<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let planned_workout_ids = planned_workout_ids.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|link| {
                    link.user_id == user_id
                        && planned_workout_ids.contains(&link.planned_workout_id)
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            PlannedCompletedWorkoutLink,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
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

    fn delete_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<(), crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            stored.lock().unwrap().retain(|existing| {
                !(existing.user_id == user_id
                    && existing.completed_workout_id == completed_workout_id)
            });
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct TestRaceRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<Race>>>,
}

impl RaceRepository for TestRaceRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, crate::domain::races::RaceError>> {
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
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, crate::domain::races::RaceError>> {
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
    ) -> crate::domain::races::BoxFuture<Result<Option<Race>, crate::domain::races::RaceError>>
    {
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

    fn upsert(
        &self,
        race: Race,
    ) -> crate::domain::races::BoxFuture<Result<Race, crate::domain::races::RaceError>> {
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
    ) -> crate::domain::races::BoxFuture<Result<(), crate::domain::races::RaceError>> {
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
struct TestSpecialDayRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<SpecialDay>>>,
}

impl SpecialDayRepository for TestSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>>
    {
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
    ) -> super::BoxFuture<Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>>
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
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> super::BoxFuture<Result<SpecialDay, crate::domain::special_days::SpecialDayError>> {
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

#[derive(Clone, Default)]
struct TestExternalSyncStateRepository {
    states: std::sync::Arc<std::sync::Mutex<Vec<ExternalSyncState>>>,
    single_lookup_count: std::sync::Arc<AtomicUsize>,
    batch_lookup_count: std::sync::Arc<AtomicUsize>,
}

impl TestExternalSyncStateRepository {
    fn with_states(states: Vec<ExternalSyncState>) -> Self {
        Self {
            states: std::sync::Arc::new(std::sync::Mutex::new(states)),
            single_lookup_count: std::sync::Arc::new(AtomicUsize::new(0)),
            batch_lookup_count: std::sync::Arc::new(AtomicUsize::new(0)),
        }
    }

    fn lookup_counts(&self) -> (usize, usize) {
        (
            self.single_lookup_count.load(Ordering::Relaxed),
            self.batch_lookup_count.load(Ordering::Relaxed),
        )
    }
}

impl ExternalSyncStateRepository for TestExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let batch_lookup_count = self.batch_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            batch_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
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
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
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
        let states = self.states.clone();
        let batch_lookup_count = self.batch_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            batch_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
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
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        Box::pin(async { Ok(()) })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_plan_id == Some(wahoo_plan_id)
                })
                .cloned())
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_workout_token.as_deref()
                            == Some(wahoo_workout_token.as_str())
                })
                .cloned())
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity.entity_kind == CanonicalEntityKind::PlannedWorkout
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }
}

#[test]
fn integrity_report_flags_missing_duplicate_type_mismatch_and_orphan_rows() {
    let expected = vec![project_planned_workout_entry(
        &sample_planned_workout(),
        &[],
    )];
    let actual = vec![
        project_completed_workout_entry(&sample_completed_workout()),
        project_completed_workout_entry(&sample_completed_workout()),
        sample_orphan_entry(),
        sample_type_mismatch_entry(),
    ];

    let report = verify_calendar_entry_integrity(&expected, &actual);

    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::MissingEntry {
            entry_id: "planned:planned-1".to_string(),
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::DuplicateEntry {
            entry_id: "completed:completed-1".to_string(),
            count: 2,
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::OrphanEntry {
            entry_id: "special:orphan-1".to_string(),
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::TypeMismatch {
            entry_id: "planned:planned-1".to_string(),
            expected_kind: CalendarEntryKind::PlannedWorkout,
            actual_kind: CalendarEntryKind::Race,
        }));
}

#[test]
fn planned_workout_projection_builds_local_entry() {
    let entry = project_planned_workout_entry(&sample_planned_workout(), &[]);

    assert_eq!(entry.entry_id, "planned:planned-1");
    assert_eq!(entry.title, "Threshold builder");
    assert_eq!(entry.description.as_deref(), Some("Classic threshold set"));
    assert_eq!(entry.planned_workout_id.as_deref(), Some("planned-1"));
    assert_eq!(entry.sync, None);
}

#[test]
fn planned_workout_projection_serializes_equal_watts_targets_in_round_trip_safe_form() {
    let workout = PlannedWorkout::new(
        "planned-watts".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![PlannedWorkoutLine::Step(PlannedWorkoutStep {
                duration_seconds: 300,
                kind: PlannedWorkoutStepKind::Steady,
                target: PlannedWorkoutTarget::WattsRange { min: 250, max: 250 },
            })],
        },
    );

    let entry = project_planned_workout_entry(&workout, &[]);

    assert_eq!(entry.raw_workout_doc.as_deref(), Some("- 5m 250-250W"));
}

#[test]
fn completed_workout_projection_carries_local_summary() {
    let entry = project_completed_workout_entry(&sample_completed_workout());

    assert_eq!(entry.entry_id, "completed:completed-1");
    assert_eq!(entry.title, "Threshold Ride");
    assert_eq!(entry.description.as_deref(), Some("Strong day"));
    assert_eq!(entry.planned_workout_id.as_deref(), Some("planned-1"));
    assert_eq!(
        entry
            .summary
            .as_ref()
            .and_then(|summary| summary.training_stress_score),
        Some(82)
    );
    assert_eq!(entry.completed_workout_id.as_deref(), Some("completed-1"));
}

#[test]
fn completed_workout_projection_handles_short_start_date_local_without_panicking() {
    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05".to_string();

    let entry = project_completed_workout_entry(&workout);

    assert_eq!(entry.date, "2026-05");
}

#[test]
fn race_projection_keeps_label_shape_and_sync_metadata() {
    let entry = project_race_entry(&sample_race(), Some(&sample_race_sync_state()));

    assert_eq!(entry.entry_id, "race:race-1");
    assert_eq!(entry.title, "Race Gravel Attack");
    assert_eq!(entry.subtitle.as_deref(), Some("120 km • Kat. B"));
    assert_eq!(entry.description, None);
    assert_eq!(
        entry.race.as_ref().map(|race| race.distance_meters),
        Some(120_000)
    );
    assert_eq!(
        entry.race.as_ref().map(|race| race.discipline.as_str()),
        Some("gravel")
    );
    assert_eq!(
        entry.race.as_ref().map(|race| race.priority.as_str()),
        Some("B")
    );
    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(41)
    );
}

#[test]
fn race_projection_handles_non_numeric_intervals_external_id_gracefully() {
    let sync_state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced(
        "not-a-number".to_string(),
        "hash-1".to_string(),
        1_700_000_000,
    );

    let entry = project_race_entry(&sample_race(), Some(&sync_state));
    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        None
    );
    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("synced")
    );
}

#[test]
fn special_day_projection_keeps_meaningful_title() {
    let entry = project_special_day_entry(&sample_special_day());

    assert_eq!(entry.entry_id, "special:special-1");
    assert_eq!(entry.title, "Flu");
    assert_eq!(entry.description.as_deref(), Some("Stay off the bike"));
    assert_eq!(entry.special_day_id.as_deref(), Some("special-1"));
}

fn sample_planned_workout() -> PlannedWorkout {
    PlannedWorkout::new(
        "planned-1".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Threshold builder".to_string(),
                }),
                PlannedWorkoutLine::Step(PlannedWorkoutStep {
                    duration_seconds: 600,
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
        Some("Threshold builder".to_string()),
        Some("Classic threshold set".to_string()),
        Some("Ride".to_string()),
    )
}

fn sample_bridged_planned_workout(operation_key: &str, date: &str) -> PlannedWorkout {
    PlannedWorkout::new(
        format!("{operation_key}:{date}"),
        "user-1".to_string(),
        date.to_string(),
        sample_planned_workout().workout,
    )
}

fn sample_completed_workout() -> CompletedWorkout {
    CompletedWorkout::new(
        "completed-1".to_string(),
        "user-1".to_string(),
        "2026-05-11T08:00:00".to_string(),
        Some("activity-1".to_string()),
        Some("planned-1".to_string()),
        Some("Threshold Ride".to_string()),
        Some("Strong day".to_string()),
        Some("Ride".to_string()),
        Some("external-1".to_string()),
        false,
        Some(3600),
        Some(35_200.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(82),
            normalized_power_watts: Some(252),
            intensity_factor: Some(0.86),
            efficiency_factor: None,
            variability_index: Some(1.05),
            average_power_watts: Some(228),
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
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["steady threshold".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z4".to_string(),
                seconds: 1400,
            }],
            heart_rate_zone_times: vec![700],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

fn sample_completed_basic_workout() -> CompletedWorkout {
    let mut workout = sample_completed_workout();
    workout.details.streams.clear();
    workout.details.interval_summary.clear();
    workout.details.power_zone_times.clear();
    workout.details.heart_rate_zone_times.clear();
    workout
}

fn sample_race() -> Race {
    Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-05-12".to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    }
}

fn sample_race_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced("41".to_string(), "hash-1".to_string(), 1_700_000_000)
}

fn sample_planned_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::PlannedWorkout, "planned-1".to_string()),
    )
    .mark_synced("77".to_string(), "hash-2".to_string(), 1_700_000_001)
}

fn sample_special_day() -> SpecialDay {
    SpecialDay::new(
        "special-1".to_string(),
        "user-1".to_string(),
        "2026-05-13".to_string(),
        SpecialDayKind::Illness,
        Some("Flu".to_string()),
        Some("Stay off the bike".to_string()),
    )
    .unwrap()
}

fn sample_other_special_day() -> SpecialDay {
    SpecialDay::new(
        "special-stale".to_string(),
        "user-1".to_string(),
        "2026-05-09".to_string(),
        SpecialDayKind::Other,
        Some("Travel".to_string()),
        Some("Airport day".to_string()),
    )
    .unwrap()
}

fn sample_calendar_entry_with_date(date: &str) -> super::CalendarEntryView {
    super::CalendarEntryView {
        entry_id: format!("special:existing-{date}"),
        user_id: "user-1".to_string(),
        entry_kind: CalendarEntryKind::SpecialDay,
        date: date.to_string(),
        start_date_local: None,
        title: "Existing entry".to_string(),
        subtitle: None,
        description: None,
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: None,
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: None,
        special_day_id: Some(format!("existing-{date}")),
        race: None,
        summary: None,
        sync: None,
    }
}

fn sample_special_day_for_user(user_id: &str) -> SpecialDay {
    SpecialDay::new(
        "special-other-user".to_string(),
        user_id.to_string(),
        "2026-05-13".to_string(),
        SpecialDayKind::Other,
        Some("Other".to_string()),
        Some("Other user note".to_string()),
    )
    .unwrap()
}

fn sample_orphan_entry() -> super::CalendarEntryView {
    project_special_day_entry(
        &SpecialDay::new(
            "orphan-1".to_string(),
            "user-1".to_string(),
            "2026-05-14".to_string(),
            SpecialDayKind::Other,
            Some("Maintenance".to_string()),
            Some("Bike in workshop".to_string()),
        )
        .unwrap(),
    )
}

fn sample_type_mismatch_entry() -> super::CalendarEntryView {
    let mut entry = project_race_entry(&sample_race(), None);
    entry.entry_id = "planned:planned-1".to_string();
    entry.entry_kind = CalendarEntryKind::Race;
    entry.planned_workout_id = Some("planned-1".to_string());
    entry.race_id = Some("race-1".to_string());
    entry
}
