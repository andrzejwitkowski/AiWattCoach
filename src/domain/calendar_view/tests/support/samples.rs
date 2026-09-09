use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    external_sync::{CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState},
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutStep,
        PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    races::{Race, RaceDiscipline, RacePriority},
    special_days::{SpecialDay, SpecialDayKind},
};

use crate::domain::calendar_view::{
    project_race_entry, project_special_day_entry, CalendarEntryKind, CalendarEntryView,
};

pub(crate) fn sample_planned_workout() -> PlannedWorkout {
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

pub(crate) fn sample_bridged_planned_workout(operation_key: &str, date: &str) -> PlannedWorkout {
    PlannedWorkout::new(
        format!("{operation_key}:{date}"),
        "user-1".to_string(),
        date.to_string(),
        sample_planned_workout().workout,
    )
}

pub(crate) fn sample_completed_workout() -> CompletedWorkout {
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

pub(crate) fn sample_completed_basic_workout() -> CompletedWorkout {
    let mut workout = sample_completed_workout();
    workout.details.streams.clear();
    workout.details.interval_summary.clear();
    workout.details.power_zone_times.clear();
    workout.details.heart_rate_zone_times.clear();
    workout
}

pub(crate) fn sample_race() -> Race {
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

pub(crate) fn sample_race_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced("41".to_string(), "hash-1".to_string(), 1_700_000_000)
}

pub(crate) fn sample_planned_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::PlannedWorkout, "planned-1".to_string()),
    )
    .mark_synced("77".to_string(), "hash-2".to_string(), 1_700_000_001)
}

pub(crate) fn sample_special_day() -> SpecialDay {
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

pub(crate) fn sample_other_special_day() -> SpecialDay {
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

pub(crate) fn sample_calendar_entry_with_date(date: &str) -> CalendarEntryView {
    CalendarEntryView {
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

pub(crate) fn sample_special_day_for_user(user_id: &str) -> SpecialDay {
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

pub(crate) fn sample_orphan_entry() -> CalendarEntryView {
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

pub(crate) fn sample_type_mismatch_entry() -> CalendarEntryView {
    let mut entry = project_race_entry(&sample_race(), None);
    entry.entry_id = "planned:planned-1".to_string();
    entry.entry_kind = CalendarEntryKind::Race;
    entry.planned_workout_id = Some("planned-1".to_string());
    entry.race_id = Some("race-1".to_string());
    entry
}
