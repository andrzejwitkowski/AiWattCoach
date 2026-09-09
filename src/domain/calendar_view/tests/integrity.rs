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
