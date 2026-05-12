use crate::domain::{
    intervals::{self, parse_planned_workout, Event, EventCategory, UpdateEvent},
    planned_workouts::{self, PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine},
    training_plan::TrainingPlanProjectedDay,
};

use super::UpdatePlannedWorkoutError;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SyncablePlannedWorkout {
    pub(super) planned_workout_id: String,
    pub(super) date: String,
    pub(super) rest_day: bool,
    pub(super) rest_day_reason: Option<String>,
    pub(super) name: Option<String>,
    pub(super) workout: crate::domain::intervals::PlannedWorkout,
}

impl From<intervals::PlannedWorkoutStepKind> for planned_workouts::PlannedWorkoutStepKind {
    fn from(value: intervals::PlannedWorkoutStepKind) -> Self {
        match value {
            intervals::PlannedWorkoutStepKind::Steady => Self::Steady,
            intervals::PlannedWorkoutStepKind::Ramp => Self::Ramp,
        }
    }
}

impl From<intervals::PlannedWorkoutTarget> for planned_workouts::PlannedWorkoutTarget {
    fn from(value: intervals::PlannedWorkoutTarget) -> Self {
        match value {
            intervals::PlannedWorkoutTarget::PercentFtp { min, max } => {
                Self::PercentFtp { min, max }
            }
            intervals::PlannedWorkoutTarget::WattsRange { min, max } => {
                Self::WattsRange { min, max }
            }
        }
    }
}

impl From<intervals::PlannedWorkoutStep> for planned_workouts::PlannedWorkoutStep {
    fn from(value: intervals::PlannedWorkoutStep) -> Self {
        Self {
            duration_seconds: value.duration_seconds,
            kind: value.kind.into(),
            target: value.target.into(),
        }
    }
}

impl From<intervals::PlannedWorkoutLine> for PlannedWorkoutLine {
    fn from(value: intervals::PlannedWorkoutLine) -> Self {
        match value {
            intervals::PlannedWorkoutLine::BlankLine => Self::BlankLine,
            intervals::PlannedWorkoutLine::Text(text) => {
                Self::Text(planned_workouts::PlannedWorkoutText { text: text.text })
            }
            intervals::PlannedWorkoutLine::Repeat(repeat) => {
                Self::Repeat(planned_workouts::PlannedWorkoutRepeat {
                    title: repeat.title,
                    count: repeat.count,
                })
            }
            intervals::PlannedWorkoutLine::Step(step) => Self::Step(step.into()),
        }
    }
}

pub(super) fn map_intervals_to_canonical_planned_workout_content(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> PlannedWorkoutContent {
    PlannedWorkoutContent {
        lines: workout.lines.iter().cloned().map(Into::into).collect(),
    }
}

pub(super) fn map_planned_workout_to_syncable(
    workout: &PlannedWorkout,
) -> Result<SyncablePlannedWorkout, UpdatePlannedWorkoutError> {
    let serialized = crate::domain::planned_workouts::serialize_canonical_planned_workout(workout);
    let parsed = parse_planned_workout(&serialized).map_err(|error| {
        UpdatePlannedWorkoutError::Validation(format!("invalid planned workout: {error}"))
    })?;
    let name = workout
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| planned_workout_name(&parsed));
    Ok(SyncablePlannedWorkout {
        planned_workout_id: workout.planned_workout_id.clone(),
        date: workout.date.clone(),
        rest_day: workout.rest_day,
        rest_day_reason: workout.rest_day_reason.clone(),
        name,
        workout: parsed,
    })
}

impl SyncablePlannedWorkout {
    pub(super) fn payload_hash(&self) -> String {
        let workout_text = if self.rest_day {
            None
        } else {
            Some(crate::domain::intervals::serialize_planned_workout_for_intervals(&self.workout))
        };
        crate::domain::planned_workouts::planned_workout_payload_hash_parts(
            &self.date,
            self.name.as_deref(),
            crate::domain::planned_workouts::comparable_workout_text_for_payload_hash(
                self.name.as_deref(),
                workout_text.as_deref(),
            )
            .as_deref(),
        )
    }

    pub(super) fn build_intervals_update(&self, existing_event: &Event) -> UpdateEvent {
        UpdateEvent {
            category: Some(EventCategory::Workout),
            start_date_local: Some(format!("{}T00:00:00", self.date)),
            event_type: existing_event
                .event_type
                .clone()
                .or_else(|| Some("Ride".to_string())),
            name: self.name.clone(),
            description: preserve_event_description(
                existing_event.description.as_deref(),
                self.sync_body().as_deref(),
                existing_event.workout_doc.as_deref(),
            ),
            indoor: Some(existing_event.indoor),
            color: existing_event.color.clone(),
            workout_doc: None,
            file_upload: None,
        }
    }

    pub(super) fn minutes(&self) -> Result<i32, UpdatePlannedWorkoutError> {
        let workout_text =
            crate::domain::intervals::serialize_planned_workout_for_intervals(&self.workout);
        let total_seconds = crate::domain::intervals::parse_workout_doc(Some(&workout_text), None)
            .summary
            .total_duration_seconds;
        if total_seconds <= 0 {
            return Err(UpdatePlannedWorkoutError::Validation(
                "planned workout has no syncable duration".to_string(),
            ));
        }
        Ok((total_seconds + 59) / 60)
    }

    pub(super) fn to_projected_day(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> TrainingPlanProjectedDay {
        TrainingPlanProjectedDay {
            user_id: user_id.to_string(),
            workout_id: self.planned_workout_id.clone(),
            operation_key: self
                .planned_workout_id
                .split(':')
                .next()
                .unwrap_or(&self.planned_workout_id)
                .to_string(),
            date: self.date.clone(),
            rest_day: self.rest_day,
            rest_day_reason: self.rest_day_reason.clone(),
            workout: Some(self.workout.clone()),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    fn sync_body(&self) -> Option<String> {
        if self.rest_day {
            return None;
        }
        let workout_text =
            crate::domain::intervals::serialize_planned_workout_for_intervals(&self.workout);
        crate::domain::planned_workouts::comparable_workout_text_for_payload_hash(
            self.name.as_deref(),
            Some(workout_text.as_str()),
        )
    }
}

pub(super) fn planned_workout_name(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> Option<String> {
    workout.lines.iter().find_map(|line| match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
        _ => None,
    })
}

pub(super) fn preserve_event_description(
    existing: Option<&str>,
    projected: Option<&str>,
    previous_workout_doc: Option<&str>,
) -> Option<String> {
    let existing = existing
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| strip_generated_workout_text(value, previous_workout_doc, projected));
    match (existing.as_deref(), projected) {
        (None, None) => None,
        (Some(existing), None) => Some(existing.to_string()),
        (None, Some(projected)) => Some(projected.to_string()),
        (Some(existing), Some(projected)) if existing.contains(projected) => {
            Some(existing.to_string())
        }
        (Some(existing), Some(projected)) => Some(format!("{existing}\n\n{projected}")),
    }
}

fn strip_generated_workout_text(
    existing: &str,
    previous_workout_doc: Option<&str>,
    projected: Option<&str>,
) -> Option<String> {
    let previous = previous_workout_doc
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| infer_legacy_generated_workout_text(existing, projected));
    let Some(previous) = previous else {
        return Some(existing.to_string());
    };
    let previous = previous.trim();
    if previous.is_empty() {
        return Some(existing.to_string());
    }
    let trimmed_existing = existing.trim_end();
    let Some(start_index) = trimmed_existing.rfind(previous) else {
        return Some(existing.to_string());
    };
    if trimmed_existing[start_index..].trim() != previous {
        return Some(existing.to_string());
    }
    let normalized = trimmed_existing[..start_index].trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn infer_legacy_generated_workout_text<'a>(
    existing: &'a str,
    projected: Option<&str>,
) -> Option<&'a str> {
    let title = projected?.lines().next()?.trim();
    if title.is_empty() {
        return None;
    }
    let start_index = existing.rfind(title)?;
    Some(existing[start_index..].trim())
}
