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

use super::super::support::*;
use crate::domain::calendar_view::ports::InMemoryCalendarEntryViewRepository;
use crate::domain::calendar_view::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, verify_calendar_entry_integrity, CalendarEntryIntegrityIssue,
    CalendarEntryKind, CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService,
    CalendarEntryViewRepository, CalendarEntryViewService, CalendarPlannedSyncKey,
    CalendarPlannedWorkoutCandidate, CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
    ManualCalendarRefreshService, ManualCalendarRefreshUseCases,
};

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
async fn refresh_range_for_user_deletes_older_imported_when_day_has_newer_timestamp() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestCalendarPlannedWorkoutSource::default();

    let mut legacy = sample_planned_workout();
    legacy.planned_workout_id = "legacy-sharpening".to_string();
    legacy.name = Some("Race-Specific Sharpening".to_string());
    legacy.updated_at_epoch_seconds = None;

    let mut newer = sample_planned_workout();
    newer.planned_workout_id = "coach-rest".to_string();
    newer.name = Some("Rest Day".to_string());
    newer.rest_day = true;
    newer.updated_at_epoch_seconds = Some(1_700_000_999);

    planned.upsert(legacy, CalendarPlannedWorkoutOrigin::Imported, vec![]);
    planned.upsert(newer, CalendarPlannedWorkoutOrigin::Imported, vec![]);

    let refresher = CalendarEntryViewRefreshService::new(
        views,
        planned.clone(),
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
    assert_eq!(refreshed[0].entry_id, "planned:coach-rest");
    let remaining = planned.stored_imported_ids();
    assert_eq!(remaining, vec!["coach-rest".to_string()]);
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
