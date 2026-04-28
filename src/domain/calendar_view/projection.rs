use crate::domain::{
    completed_workouts::CompletedWorkout,
    external_sync::{ExternalProvider, ExternalSyncState, ExternalSyncStatus},
    planned_workouts::PlannedWorkout,
    races::Race,
    special_days::{SpecialDay, SpecialDayKind},
};
use sha2::{Digest, Sha256};

use super::{
    CalendarEntryKind, CalendarEntryRace, CalendarEntrySummary, CalendarEntrySync,
    CalendarEntryView,
};

pub fn project_planned_workout_entry(
    workout: &PlannedWorkout,
    sync_states: &[ExternalSyncState],
) -> CalendarEntryView {
    let raw_workout_doc = if workout.rest_day {
        None
    } else {
        Some(serialize_planned_workout(workout))
    };

    CalendarEntryView {
        entry_id: format!("planned:{}", workout.planned_workout_id),
        user_id: workout.user_id.clone(),
        entry_kind: CalendarEntryKind::PlannedWorkout,
        date: workout.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", workout.date)),
        title: workout
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| planned_workout_title(workout)),
        subtitle: if workout.rest_day {
            None
        } else {
            Some(format!("{} lines", workout.workout.lines.len()))
        },
        description: workout.description.clone(),
        rest_day: workout.rest_day,
        rest_day_reason: workout.rest_day_reason.clone(),
        raw_workout_doc,
        planned_workout_id: Some(workout.planned_workout_id.clone()),
        completed_workout_id: None,
        race_id: None,
        special_day_id: None,
        race: None,
        summary: None,
        sync: map_planned_workout_sync_states(workout, sync_states),
    }
}

pub fn project_completed_workout_entry(workout: &CompletedWorkout) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("completed:{}", workout.completed_workout_id),
        user_id: workout.user_id.clone(),
        entry_kind: CalendarEntryKind::CompletedWorkout,
        date: date_prefix(&workout.start_date_local).to_string(),
        start_date_local: Some(workout.start_date_local.clone()),
        title: workout
            .name
            .clone()
            .unwrap_or_else(|| "Completed workout".to_string()),
        subtitle: workout
            .metrics
            .training_stress_score
            .map(|tss| format!("TSS {tss}")),
        description: workout
            .description
            .clone()
            .or_else(|| workout.details.interval_summary.first().cloned()),
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: None,
        planned_workout_id: workout.planned_workout_id.clone(),
        completed_workout_id: Some(workout.completed_workout_id.clone()),
        race_id: None,
        special_day_id: None,
        race: None,
        summary: Some(CalendarEntrySummary {
            training_stress_score: workout.metrics.training_stress_score,
            intensity_factor: workout.metrics.intensity_factor,
            normalized_power_watts: workout.metrics.normalized_power_watts,
        }),
        sync: None,
    }
}

pub fn project_race_entry(
    race: &Race,
    sync_state: Option<&ExternalSyncState>,
) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("race:{}", race.race_id),
        user_id: race.user_id.clone(),
        entry_kind: CalendarEntryKind::Race,
        date: race.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", race.date)),
        title: race.label_title(),
        subtitle: Some(race.label_subtitle()),
        description: None,
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: None,
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: Some(race.race_id.clone()),
        special_day_id: None,
        race: Some(CalendarEntryRace {
            distance_meters: race.distance_meters,
            discipline: race.discipline.as_str().to_string(),
            priority: race.priority.as_str().to_string(),
        }),
        summary: None,
        sync: map_sync_state(sync_state),
    }
}

pub fn project_special_day_entry(special_day: &SpecialDay) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("special:{}", special_day.special_day_id),
        user_id: special_day.user_id.clone(),
        entry_kind: CalendarEntryKind::SpecialDay,
        date: special_day.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", special_day.date)),
        title: special_day
            .title
            .clone()
            .unwrap_or_else(|| special_day_title(&special_day.kind)),
        subtitle: None,
        description: special_day.description.clone(),
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: None,
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: None,
        special_day_id: Some(special_day.special_day_id.clone()),
        race: None,
        summary: None,
        sync: None,
    }
}

fn planned_workout_title(workout: &PlannedWorkout) -> String {
    if workout.rest_day {
        return "Rest Day".to_string();
    }

    workout
        .workout
        .lines
        .iter()
        .find_map(|line| match line {
            crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
                Some(text.text.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| "Planned workout".to_string())
}

fn serialize_planned_workout(workout: &PlannedWorkout) -> String {
    let structured = crate::domain::intervals::PlannedWorkout {
        lines: workout
            .workout
            .lines
            .iter()
            .cloned()
            .map(map_canonical_line_to_intervals_line)
            .collect(),
    };

    crate::domain::intervals::serialize_planned_workout(&structured)
}

fn map_canonical_line_to_intervals_line(
    line: crate::domain::planned_workouts::PlannedWorkoutLine,
) -> crate::domain::intervals::PlannedWorkoutLine {
    match line {
        crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
            crate::domain::intervals::PlannedWorkoutLine::Text(
                crate::domain::intervals::PlannedWorkoutText { text: text.text },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Repeat(repeat) => {
            crate::domain::intervals::PlannedWorkoutLine::Repeat(
                crate::domain::intervals::PlannedWorkoutRepeat {
                    title: repeat.title,
                    count: repeat.count,
                },
            )
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Step(step) => {
            crate::domain::intervals::PlannedWorkoutLine::Step(
                crate::domain::intervals::PlannedWorkoutStep {
                    duration_seconds: step.duration_seconds,
                    kind: match step.kind {
                        crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady => {
                            crate::domain::intervals::PlannedWorkoutStepKind::Steady
                        }
                        crate::domain::planned_workouts::PlannedWorkoutStepKind::Ramp => {
                            crate::domain::intervals::PlannedWorkoutStepKind::Ramp
                        }
                    },
                    target: match step.target {
                        crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp {
                            min,
                            max,
                        } => {
                            crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max }
                        }
                        crate::domain::planned_workouts::PlannedWorkoutTarget::WattsRange {
                            min,
                            max,
                        } => {
                            crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max }
                        }
                    },
                },
            )
        }
    }
}

fn special_day_title(kind: &SpecialDayKind) -> String {
    match kind {
        SpecialDayKind::Illness => "Illness".to_string(),
        SpecialDayKind::Travel => "Travel".to_string(),
        SpecialDayKind::Blocked => "Blocked day".to_string(),
        SpecialDayKind::Note => "Note".to_string(),
        SpecialDayKind::Other => "Special day".to_string(),
    }
}

fn map_sync_state(sync_state: Option<&ExternalSyncState>) -> Option<CalendarEntrySync> {
    sync_state.map(|state| CalendarEntrySync {
        linked_intervals_event_id: linked_intervals_event_id(state),
        sync_status: Some(state.sync_status.as_str().to_string()),
    })
}

fn map_planned_workout_sync_states(
    workout: &PlannedWorkout,
    sync_states: &[ExternalSyncState],
) -> Option<CalendarEntrySync> {
    if sync_states.is_empty() {
        return None;
    }

    if sync_states.iter().any(|state| {
        state
            .last_synced_payload_hash
            .as_deref()
            .is_some_and(|hash| hash != current_planned_payload_hash(workout))
    }) {
        let linked_intervals_event_id = sync_states
            .iter()
            .find(|state| state.provider == ExternalProvider::Intervals)
            .and_then(linked_intervals_event_id);

        return Some(CalendarEntrySync {
            linked_intervals_event_id,
            sync_status: Some("modified".to_string()),
        });
    }

    let sync_status = if sync_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Synced)
    {
        "synced"
    } else if sync_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Pending)
    {
        "pending"
    } else if sync_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Failed)
    {
        "failed"
    } else {
        return None;
    };

    let linked_intervals_event_id = sync_states
        .iter()
        .find(|state| state.provider == ExternalProvider::Intervals)
        .and_then(linked_intervals_event_id);

    Some(CalendarEntrySync {
        linked_intervals_event_id,
        sync_status: Some(sync_status.to_string()),
    })
}

fn current_planned_payload_hash(workout: &PlannedWorkout) -> String {
    let workout_name = planned_workout_sync_name(workout);
    let workout_body = planned_workout_sync_body(workout);
    let digest = Sha256::digest(format!(
        "{}\n{}\n{}",
        workout.date,
        workout_name.as_deref().unwrap_or_default(),
        workout_body.as_deref().unwrap_or_default()
    ));
    format!("{digest:x}")
}

fn planned_workout_sync_name(workout: &PlannedWorkout) -> Option<String> {
    if workout.rest_day {
        return Some("Rest Day".to_string());
    }

    workout.workout.lines.iter().find_map(|line| match line {
        crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
        _ => None,
    })
}

fn planned_workout_sync_body(workout: &PlannedWorkout) -> Option<String> {
    if workout.rest_day {
        return None;
    }

    let workout_name = planned_workout_sync_name(workout);
    let lines = workout
        .workout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let is_title_line = index == 0
                && matches!(line, crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) if workout_name.as_deref().is_some_and(|name| text.text == name));

            (!is_title_line).then(|| serialize_planned_workout_line(line))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        workout_name
    } else {
        Some(lines.join("\n"))
    }
}

fn serialize_planned_workout_line(
    line: &crate::domain::planned_workouts::PlannedWorkoutLine,
) -> String {
    match line {
        crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => text.text.clone(),
        crate::domain::planned_workouts::PlannedWorkoutLine::Repeat(repeat) => {
            match &repeat.title {
                Some(title) if !title.trim().is_empty() => format!("{title} {}x", repeat.count),
                _ => format!("{}x", repeat.count),
            }
        }
        crate::domain::planned_workouts::PlannedWorkoutLine::Step(step) => {
            let duration_label = format_projected_step_duration(step.duration_seconds);
            let ramp_label = match step.kind {
                crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady => "",
                crate::domain::planned_workouts::PlannedWorkoutStepKind::Ramp => " ramp",
            };
            let target_label = match step.target {
                crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp { min, max } => {
                    if (min - max).abs() < f64::EPSILON {
                        format!("{}%", trim_decimal(min))
                    } else {
                        format!("{}-{}%", trim_decimal(min), trim_decimal(max))
                    }
                }
                crate::domain::planned_workouts::PlannedWorkoutTarget::WattsRange { min, max } => {
                    if min == max {
                        format!("{min}W")
                    } else {
                        format!("{min}-{max}W")
                    }
                }
            };

            format!("- {duration_label}{ramp_label} {target_label}")
        }
    }
}

fn format_projected_step_duration(duration_seconds: i32) -> String {
    if duration_seconds % 60 == 0 {
        format!("{}m", duration_seconds / 60)
    } else {
        format!("{duration_seconds}s")
    }
}

fn trim_decimal(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

fn linked_intervals_event_id(state: &ExternalSyncState) -> Option<i64> {
    if state.provider != ExternalProvider::Intervals {
        return None;
    }

    state.external_id.as_deref().map(|value| {
        value.parse::<i64>().unwrap_or_else(|error| {
            panic!("intervals sync state external_id must parse as i64, got '{value}': {error}")
        })
    })
}

fn date_prefix(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}
