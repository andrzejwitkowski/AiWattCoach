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

pub fn to_intervals_planned_workout(
    workout: &PlannedWorkout,
) -> Result<
    crate::domain::intervals::PlannedWorkout,
    crate::domain::intervals::PlannedWorkoutParseError,
> {
    let serialized = serialize_canonical_planned_workout(workout);
    crate::domain::intervals::parse_planned_workout(&serialized)
}

pub fn intervals_planned_workout_payload_hash(
    date: &str,
    workout: &crate::domain::intervals::PlannedWorkout,
    name: Option<&str>,
) -> String {
    let resolved_name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| intervals_planned_workout_title(workout));
    let body = intervals_planned_workout_sync_body(workout, resolved_name.as_deref());
    planned_workout_payload_hash_parts(date, resolved_name.as_deref(), body.as_deref())
}

pub fn domain_to_intervals_planned_workout(
    workout: &PlannedWorkout,
) -> crate::domain::intervals::PlannedWorkout {
    crate::domain::intervals::PlannedWorkout {
        lines: workout
            .workout
            .lines
            .iter()
            .cloned()
            .map(map_canonical_line_to_intervals_line)
            .collect(),
    }
}

pub fn planned_workout_sync_name(workout: &PlannedWorkout) -> Option<String> {
    if workout.rest_day {
        return Some("Rest Day".to_string());
    }

    workout
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| intervals_planned_workout_title(&domain_to_intervals_planned_workout(workout)))
}

pub fn planned_workout_payload_hash(workout: &PlannedWorkout) -> String {
    if workout.rest_day {
        return planned_workout_payload_hash_parts(&workout.date, Some("Rest Day"), None);
    }

    let parsed = domain_to_intervals_planned_workout(workout);
    intervals_planned_workout_payload_hash(
        &workout.date,
        &parsed,
        planned_workout_sync_name(workout).as_deref(),
    )
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

fn intervals_planned_workout_title(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> Option<String> {
    workout.lines.iter().find_map(|line| match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
        _ => None,
    })
}

fn intervals_planned_workout_sync_body(
    workout: &crate::domain::intervals::PlannedWorkout,
    name: Option<&str>,
) -> Option<String> {
    let serialized = crate::domain::intervals::serialize_planned_workout_for_intervals(workout);
    comparable_workout_text_for_payload_hash(name, Some(serialized.as_str()))
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
