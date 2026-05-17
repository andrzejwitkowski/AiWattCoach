use crate::domain::{
    intervals::{self, Event, EventCategory, UpdateEvent},
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
    let parsed = crate::domain::planned_workouts::to_intervals_planned_workout(workout).map_err(
        |error| UpdatePlannedWorkoutError::Validation(format!("invalid planned workout: {error}")),
    )?;
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
            supervisor_status: None,
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

#[cfg(test)]
mod tests {
    use super::{infer_legacy_generated_workout_text, strip_generated_workout_text};

    #[test]
    fn strip_returns_existing_when_previous_and_projected_absent() {
        assert_eq!(
            strip_generated_workout_text("coach note", None, None),
            Some("coach note".to_string())
        );
    }

    #[test]
    fn strip_returns_existing_when_previous_doc_is_whitespace_only_and_projected_absent() {
        assert_eq!(
            strip_generated_workout_text("coach note", Some("   \n  "), None),
            Some("coach note".to_string())
        );
    }

    #[test]
    fn strip_returns_existing_when_previous_doc_not_found_in_existing() {
        assert_eq!(
            strip_generated_workout_text("coach note", Some("something else"), None),
            Some("coach note".to_string())
        );
    }

    #[test]
    fn strip_removes_previous_workout_doc_suffix() {
        assert_eq!(
            strip_generated_workout_text(
                "coach note\n\nOld workout\n- 10m 60%",
                Some("Old workout\n- 10m 60%"),
                None,
            ),
            Some("coach note".to_string())
        );
    }

    #[test]
    fn strip_returns_none_when_existing_is_only_previous_workout_doc() {
        assert_eq!(
            strip_generated_workout_text(
                "Old workout\n- 10m 60%",
                Some("Old workout\n- 10m 60%"),
                None,
            ),
            None
        );
    }

    #[test]
    fn strip_does_not_remove_when_trailing_text_follows_previous_block() {
        assert_eq!(
            strip_generated_workout_text(
                "coach note\n\nOld workout\n- 10m 60%\n\nfollow up note",
                Some("Old workout\n- 10m 60%"),
                None,
            ),
            Some("coach note\n\nOld workout\n- 10m 60%\n\nfollow up note".to_string())
        );
    }

    #[test]
    fn strip_uses_last_occurrence_of_previous_block() {
        assert_eq!(
            strip_generated_workout_text(
                "coach note Old workout intro\n\nOld workout",
                Some("Old workout"),
                None,
            ),
            Some("coach note Old workout intro".to_string())
        );
    }

    #[test]
    fn strip_falls_back_to_legacy_inference_when_previous_doc_missing() {
        assert_eq!(
            strip_generated_workout_text(
                "coach note\n\nNew workout\n- 10m 60%",
                None,
                Some("New workout\n- 20m 70%"),
            ),
            Some("coach note".to_string())
        );
    }

    #[test]
    fn infer_legacy_returns_none_when_projected_is_none() {
        assert_eq!(
            infer_legacy_generated_workout_text("coach note", None),
            None
        );
    }

    #[test]
    fn infer_legacy_returns_none_when_projected_first_line_is_empty() {
        assert_eq!(
            infer_legacy_generated_workout_text("coach note", Some("   \n- 10m 60%")),
            None
        );
    }

    #[test]
    fn infer_legacy_returns_none_when_title_not_found_in_existing() {
        assert_eq!(
            infer_legacy_generated_workout_text("coach note", Some("New workout\n- 10m 60%")),
            None
        );
    }

    #[test]
    fn infer_legacy_returns_trailing_block_starting_at_last_title_occurrence() {
        assert_eq!(
            infer_legacy_generated_workout_text(
                "intro mentions New workout\n\nNew workout\n- 10m 60%",
                Some("New workout\n- 20m 70%"),
            ),
            Some("New workout\n- 10m 60%")
        );
    }
}
