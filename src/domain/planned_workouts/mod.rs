mod authoritative;
mod model;
mod ports;
#[cfg(test)]
mod tests;
mod update;

use sha2::Digest;

pub use authoritative::AuthoritativePlannedWorkoutRepository;
pub use model::{
    PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
    PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
    PlannedWorkoutText,
};
pub use ports::{BoxFuture, NoopPlannedWorkoutRepository, PlannedWorkoutRepository};
pub use update::{
    PlannedWorkoutUpdateService, ProviderSyncFailure, UpdatePlannedWorkoutCommand,
    UpdatePlannedWorkoutError, UpdatePlannedWorkoutOutcome,
};

pub fn serialize_canonical_planned_workout(workout: &PlannedWorkout) -> String {
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

pub fn planned_workout_payload_hash(workout: &PlannedWorkout) -> String {
    let name = planned_workout_sync_name(workout);
    let body = planned_workout_sync_body(workout);
    planned_workout_payload_hash_parts(&workout.date, name.as_deref(), body.as_deref())
}

pub fn planned_workout_payload_hash_parts(
    date: &str,
    name: Option<&str>,
    workout_body: Option<&str>,
) -> String {
    let digest = sha2::Sha256::digest(format!(
        "{}\n{}\n{}",
        date,
        name.unwrap_or_default(),
        workout_body.unwrap_or_default()
    ));
    format!("{digest:x}")
}

pub fn comparable_workout_text_for_payload_hash(
    name: Option<&str>,
    workout_text: Option<&str>,
) -> Option<String> {
    let workout_text = workout_text
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let Some(name) = name.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(workout_text.to_string());
    };

    let mut lines = workout_text.lines();
    let Some(first_line) = lines.next() else {
        return Some(workout_text.to_string());
    };
    if first_line.trim() != name {
        return Some(workout_text.to_string());
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        Some(name.to_string())
    } else {
        Some(body)
    }
}

fn planned_workout_sync_name(workout: &PlannedWorkout) -> Option<String> {
    if workout.rest_day {
        return Some("Rest Day".to_string());
    }

    workout.workout.lines.iter().find_map(|line| match line {
        PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
        _ => None,
    })
}

fn planned_workout_sync_body(workout: &PlannedWorkout) -> Option<String> {
    if workout.rest_day {
        return None;
    }

    let serialized = serialize_canonical_planned_workout(workout);
    comparable_workout_text_for_payload_hash(
        planned_workout_sync_name(workout).as_deref(),
        Some(&serialized),
    )
}

fn map_canonical_line_to_intervals_line(
    line: PlannedWorkoutLine,
) -> crate::domain::intervals::PlannedWorkoutLine {
    match line {
        PlannedWorkoutLine::BlankLine => crate::domain::intervals::PlannedWorkoutLine::BlankLine,
        PlannedWorkoutLine::Text(text) => crate::domain::intervals::PlannedWorkoutLine::Text(
            crate::domain::intervals::PlannedWorkoutText { text: text.text },
        ),
        PlannedWorkoutLine::Repeat(repeat) => crate::domain::intervals::PlannedWorkoutLine::Repeat(
            crate::domain::intervals::PlannedWorkoutRepeat {
                title: repeat.title,
                count: repeat.count,
            },
        ),
        PlannedWorkoutLine::Step(step) => crate::domain::intervals::PlannedWorkoutLine::Step(
            crate::domain::intervals::PlannedWorkoutStep {
                duration_seconds: step.duration_seconds,
                kind: match step.kind {
                    PlannedWorkoutStepKind::Steady => {
                        crate::domain::intervals::PlannedWorkoutStepKind::Steady
                    }
                    PlannedWorkoutStepKind::Ramp => {
                        crate::domain::intervals::PlannedWorkoutStepKind::Ramp
                    }
                },
                target: match step.target {
                    PlannedWorkoutTarget::PercentFtp { min, max } => {
                        crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max }
                    }
                    PlannedWorkoutTarget::WattsRange { min, max } => {
                        crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max }
                    }
                },
            },
        ),
    }
}
