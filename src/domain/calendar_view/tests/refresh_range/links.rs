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
