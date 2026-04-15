use sha2::{Digest, Sha256};

use crate::domain::{
    calendar::{
        CalendarEvent, CalendarEventCategory, CalendarEventSource, CalendarProjectedWorkout,
        PlannedWorkoutSyncRecord, PlannedWorkoutSyncStatus,
    },
    intervals::{Event, UpdateEvent},
    training_plan::TrainingPlanProjectedDay,
};

pub(super) fn build_projected_calendar_event(
    day: TrainingPlanProjectedDay,
    sync_record: Option<&PlannedWorkoutSyncRecord>,
) -> CalendarEvent {
    let payload_hash = projected_day_payload_hash(&day);
    let linked_intervals_event_id = sync_record.and_then(|record| record.intervals_event_id);
    let status = projected_day_sync_status(sync_record, &payload_hash);
    let event_id = linked_intervals_event_id
        .unwrap_or_else(|| synthetic_event_id(&day.operation_key, &day.date));
    let projected_workout_id = projected_workout_id(&day.operation_key, &day.date);

    CalendarEvent {
        id: event_id,
        calendar_entry_id: format!("predicted:{projected_workout_id}"),
        start_date_local: day.date.clone(),
        name: projected_workout_name(&day),
        category: CalendarEventCategory::Workout,
        description: None,
        indoor: false,
        color: None,
        raw_workout_doc: day.workout.as_ref().map(serialize_projected_workout),
        source: CalendarEventSource::Predicted,
        projected_workout: Some(CalendarProjectedWorkout {
            projected_workout_id,
            operation_key: day.operation_key.clone(),
            date: day.date.clone(),
            source_workout_id: day.workout_id,
        }),
        sync_status: Some(status),
        linked_intervals_event_id,
        actual_workout: None,
    }
}

pub(super) fn build_update_event(
    day: &TrainingPlanProjectedDay,
    existing_event: &Event,
    planned_workout_marker: Option<&str>,
) -> UpdateEvent {
    UpdateEvent {
        category: Some(crate::domain::intervals::EventCategory::Workout),
        start_date_local: Some(projected_event_start_date_local(&day.date)),
        event_type: existing_event
            .event_type
            .clone()
            .or_else(|| Some("Ride".to_string())),
        name: projected_workout_name(day),
        description: merge_event_description(
            existing_event.description.as_deref(),
            projected_workout_sync_body(day).as_deref(),
            planned_workout_marker,
        ),
        indoor: Some(existing_event.indoor),
        color: existing_event.color.clone(),
        workout_doc: None,
        file_upload: None,
    }
}

pub(super) fn projected_workout_name(day: &TrainingPlanProjectedDay) -> Option<String> {
    day.workout.as_ref().and_then(|workout| {
        workout
            .lines
            .iter()
            .find_map(|line| line.text().map(ToString::to_string))
    })
}

pub(super) fn serialize_projected_workout(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> String {
    crate::domain::intervals::serialize_planned_workout(workout)
}

pub(super) fn projected_day_sync_status(
    sync_record: Option<&PlannedWorkoutSyncRecord>,
    payload_hash: &str,
) -> PlannedWorkoutSyncStatus {
    match sync_record {
        None => PlannedWorkoutSyncStatus::Unsynced,
        Some(record)
            if record
                .synced_payload_hash
                .as_deref()
                .is_some_and(|hash| hash != payload_hash) =>
        {
            PlannedWorkoutSyncStatus::Modified
        }
        Some(record) => match record.status {
            PlannedWorkoutSyncStatus::Pending => PlannedWorkoutSyncStatus::Pending,
            PlannedWorkoutSyncStatus::Failed => PlannedWorkoutSyncStatus::Failed,
            PlannedWorkoutSyncStatus::Synced => PlannedWorkoutSyncStatus::Synced,
            PlannedWorkoutSyncStatus::Modified => PlannedWorkoutSyncStatus::Modified,
            PlannedWorkoutSyncStatus::Unsynced => PlannedWorkoutSyncStatus::Unsynced,
        },
    }
}

pub(super) fn projected_day_payload_hash(day: &TrainingPlanProjectedDay) -> String {
    projected_event_payload_hash(
        &day.date,
        projected_workout_name(day).as_deref(),
        projected_workout_sync_body(day).as_deref(),
    )
}

pub(super) fn projected_event_payload_hash(
    date: &str,
    name: Option<&str>,
    workout_doc: Option<&str>,
) -> String {
    let digest = Sha256::digest(format!(
        "{date}\n{}\n{}",
        name.unwrap_or_default(),
        workout_doc.unwrap_or_default()
    ));
    format!("{digest:x}")
}

pub fn projected_workout_id(operation_key: &str, date: &str) -> String {
    format!("{operation_key}:{date}")
}

pub fn synthetic_event_id(operation_key: &str, date: &str) -> i64 {
    const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let digest = Sha256::digest(format!("{operation_key}:{date}"));
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    ((value % MAX_JS_SAFE_INTEGER) + 1) as i64
}

pub(super) fn projected_event_start_date_local(date: &str) -> String {
    format!("{date}T00:00:00")
}

pub(super) fn projected_workout_sync_body(day: &TrainingPlanProjectedDay) -> Option<String> {
    let workout = day.workout.as_ref()?;
    let workout_name = projected_workout_name(day);

    let lines = workout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let is_title_line = index == 0
                && line
                    .text()
                    .zip(workout_name.as_deref())
                    .is_some_and(|(text, name)| text == name);

            (!is_title_line).then(|| serialize_projected_workout_line(line))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        workout_name
    } else {
        Some(lines.join("\n"))
    }
}

pub(super) fn projected_workout_sync_description(
    day: &TrainingPlanProjectedDay,
    planned_workout_marker: Option<&str>,
) -> Option<String> {
    let description = projected_workout_sync_body(day);

    match planned_workout_marker {
        Some(marker) => crate::domain::planned_workout_tokens::append_marker_to_description(
            description.as_deref(),
            marker,
        ),
        None => description,
    }
}

fn serialize_projected_workout_line(line: &crate::domain::intervals::PlannedWorkoutLine) -> String {
    match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => text.text.clone(),
        crate::domain::intervals::PlannedWorkoutLine::Repeat(repeat) => match &repeat.title {
            Some(title) => format!("{title} {}x", repeat.count),
            None => format!("{}x", repeat.count),
        },
        crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
            let duration = format_projected_step_duration(step.duration_seconds);
            let target = format_projected_step_target(&step.target);
            match step.kind {
                crate::domain::intervals::PlannedWorkoutStepKind::Steady => {
                    format!("- {duration} {target}")
                }
                crate::domain::intervals::PlannedWorkoutStepKind::Ramp => {
                    format!("- {duration} ramp {target}")
                }
            }
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

fn format_projected_step_target(target: &crate::domain::intervals::PlannedWorkoutTarget) -> String {
    match target {
        crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max } => {
            if (min - max).abs() < f64::EPSILON {
                format!("{}%", trim_decimal(*min))
            } else {
                format!("{}-{}%", trim_decimal(*min), trim_decimal(*max))
            }
        }
        crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max } => {
            if min == max {
                format!("{min}W")
            } else {
                format!("{min}-{max}W")
            }
        }
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

pub(super) fn merge_event_description(
    existing: Option<&str>,
    projected: Option<&str>,
    planned_workout_marker: Option<&str>,
) -> Option<String> {
    let merged = match (
        existing.map(str::trim).filter(|value| !value.is_empty()),
        projected,
    ) {
        (None, None) => None,
        (Some(existing), None) => Some(existing.to_string()),
        (None, Some(projected)) => Some(projected.to_string()),
        (Some(existing), Some(projected)) if existing.contains(projected) => {
            Some(existing.to_string())
        }
        (Some(existing), Some(projected)) => Some(format!("{existing}\n\n{projected}")),
    };

    match planned_workout_marker {
        Some(marker) => crate::domain::planned_workout_tokens::append_marker_to_description(
            merged.as_deref(),
            marker,
        ),
        None => merged,
    }
}
