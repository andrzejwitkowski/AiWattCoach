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
async fn manual_calendar_refresh_runs_orphan_race_projection_cleanup_before_rebuild() {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    use super::super::OrphanRaceProjectionCleanupPort;

    type OrphanCleanupCall = (String, String, String, BTreeSet<String>);

    #[derive(Clone, Default)]
    struct RecordingOrphanCleanup {
        calls: Arc<Mutex<Vec<OrphanCleanupCall>>>,
    }

    impl OrphanRaceProjectionCleanupPort for RecordingOrphanCleanup {
        fn supersede_orphan_race_projections(
            &self,
            user_id: &str,
            oldest: &str,
            newest: &str,
            race_dates_present: &BTreeSet<String>,
        ) -> super::super::BoxFuture<Result<(), super::super::CalendarEntryViewError>> {
            let calls = self.calls.clone();
            let user_id = user_id.to_string();
            let oldest = oldest.to_string();
            let newest = newest.to_string();
            let race_dates_present = race_dates_present.clone();
            Box::pin(async move {
                calls
                    .lock()
                    .unwrap()
                    .push((user_id, oldest, newest, race_dates_present));
                Ok(())
            })
        }
    }

    let races = TestRaceRepository::default();
    races.upsert(sample_race()).await.unwrap();
    let cleanup = RecordingOrphanCleanup::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = ManualCalendarRefreshService::new(
        InMemoryCalendarEntryViewRepository::default(),
        TestCalendarPlannedWorkoutSource::default(),
        TestCompletedWorkoutRepository::default(),
        races,
        TestSpecialDayRepository::default(),
        FixedClock(1_777_248_000),
        refresh.clone(),
    )
    .with_orphan_race_projection_cleanup(cleanup.clone());

    service
        .refresh_calendar_view_for_user("user-1")
        .await
        .unwrap();

    let calls = cleanup.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "user-1");
    assert!(calls[0].3.contains(&sample_race().date));
    assert_eq!(refresh.calls().len(), 1);
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
