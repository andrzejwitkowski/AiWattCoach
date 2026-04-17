use crate::domain::{
    completed_workouts::CompletedWorkout,
    external_sync::{ExternalProvider, ExternalSyncState},
    planned_workouts::PlannedWorkout,
    races::Race,
    special_days::{SpecialDay, SpecialDayKind},
};

use super::{
    CalendarEntryKind, CalendarEntryRace, CalendarEntrySummary, CalendarEntrySync,
    CalendarEntryView,
};

pub fn project_planned_workout_entry(
    workout: &PlannedWorkout,
    sync_state: Option<&ExternalSyncState>,
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
        sync: map_sync_state(sync_state),
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
