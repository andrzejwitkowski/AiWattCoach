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
    let payload_hash = crate::domain::planned_workouts::planned_workout_payload_hash(&workout);
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
