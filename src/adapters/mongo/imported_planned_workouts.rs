use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};

use crate::domain::planned_workouts::{
    PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
    PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
    PlannedWorkoutText,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ImportedPlannedWorkoutDocument {
    pub(crate) user_id: String,
    pub(crate) planned_workout_id: String,
    pub(crate) date: String,
    #[serde(default)]
    pub(crate) rest_day: bool,
    #[serde(default)]
    pub(crate) rest_day_reason: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) event_type: Option<String>,
    pub(crate) workout: StoredPlannedWorkoutContentDocument,
    #[serde(default)]
    pub(crate) updated_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StoredPlannedWorkoutContentDocument {
    pub(crate) lines: Vec<StoredPlannedWorkoutLineDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StoredPlannedWorkoutLineDocument {
    BlankLine,
    Text {
        text: String,
    },
    Repeat {
        title: Option<String>,
        count: i64,
    },
    Step {
        duration_seconds: i32,
        step_kind: String,
        target: StoredPlannedWorkoutTargetDocument,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StoredPlannedWorkoutTargetDocument {
    PercentFtp { min: f64, max: f64 },
    WattsRange { min: i32, max: i32 },
}

pub(crate) async fn delete_imported_planned_workouts_for_user_date_keeping<T>(
    imported_collection: &Collection<T>,
    user_id: &str,
    date: &str,
    keep_planned_workout_ids: Vec<String>,
) -> Result<u64, PlannedWorkoutError>
where
    T: Send + Sync,
{
    let result = imported_collection
        .delete_many(doc! {
            "user_id": user_id,
            "date": date,
            "planned_workout_id": { "$nin": keep_planned_workout_ids },
        })
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
    Ok(result.deleted_count)
}

pub(crate) fn map_imported_workout_to_document(
    workout: &PlannedWorkout,
) -> ImportedPlannedWorkoutDocument {
    ImportedPlannedWorkoutDocument {
        user_id: workout.user_id.clone(),
        planned_workout_id: workout.planned_workout_id.clone(),
        date: workout.date.clone(),
        rest_day: workout.rest_day,
        rest_day_reason: workout.rest_day_reason.clone(),
        name: workout.name.clone(),
        description: workout.description.clone(),
        event_type: workout.event_type.clone(),
        workout: StoredPlannedWorkoutContentDocument {
            lines: workout
                .workout
                .lines
                .iter()
                .map(map_domain_line_to_stored_line)
                .collect(),
        },
        updated_at_epoch_seconds: workout.updated_at_epoch_seconds,
    }
}

pub(crate) fn map_imported_document_to_domain(
    document: ImportedPlannedWorkoutDocument,
) -> Result<PlannedWorkout, PlannedWorkoutError> {
    let planned_workout = PlannedWorkout::new(
        document.planned_workout_id,
        document.user_id,
        document.date,
        PlannedWorkoutContent {
            lines: document
                .workout
                .lines
                .into_iter()
                .map(map_stored_line_to_domain)
                .collect::<Result<Vec<_>, _>>()?,
        },
    )
    .with_event_metadata(document.name, document.description, document.event_type)
    .with_updated_at(document.updated_at_epoch_seconds);

    if document.rest_day {
        Ok(planned_workout.as_rest_day(document.rest_day_reason))
    } else {
        Ok(planned_workout)
    }
}

fn map_domain_line_to_stored_line(line: &PlannedWorkoutLine) -> StoredPlannedWorkoutLineDocument {
    match line {
        PlannedWorkoutLine::BlankLine => StoredPlannedWorkoutLineDocument::BlankLine,
        PlannedWorkoutLine::Text(text) => StoredPlannedWorkoutLineDocument::Text {
            text: text.text.clone(),
        },
        PlannedWorkoutLine::Repeat(repeat) => StoredPlannedWorkoutLineDocument::Repeat {
            title: repeat.title.clone(),
            count: repeat.count as i64,
        },
        PlannedWorkoutLine::Step(step) => StoredPlannedWorkoutLineDocument::Step {
            duration_seconds: step.duration_seconds,
            step_kind: map_step_kind_to_str(&step.kind).to_string(),
            target: map_domain_target_to_stored_target(&step.target),
        },
    }
}

fn map_stored_line_to_domain(
    line: StoredPlannedWorkoutLineDocument,
) -> Result<PlannedWorkoutLine, PlannedWorkoutError> {
    match line {
        StoredPlannedWorkoutLineDocument::BlankLine => Ok(PlannedWorkoutLine::BlankLine),
        StoredPlannedWorkoutLineDocument::Text { text } => {
            Ok(PlannedWorkoutLine::Text(PlannedWorkoutText { text }))
        }
        StoredPlannedWorkoutLineDocument::Repeat { title, count } => {
            let count = usize::try_from(count).map_err(|_| {
                PlannedWorkoutError::Repository(
                    "stored planned workout repeat count cannot be negative".to_string(),
                )
            })?;
            Ok(PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat {
                title,
                count,
            }))
        }
        StoredPlannedWorkoutLineDocument::Step {
            duration_seconds,
            step_kind,
            target,
        } => Ok(PlannedWorkoutLine::Step(PlannedWorkoutStep {
            duration_seconds,
            kind: map_stored_step_kind(&step_kind)?,
            target: map_stored_target_to_domain(target),
        })),
    }
}

fn map_domain_target_to_stored_target(
    target: &PlannedWorkoutTarget,
) -> StoredPlannedWorkoutTargetDocument {
    match target {
        PlannedWorkoutTarget::PercentFtp { min, max } => {
            StoredPlannedWorkoutTargetDocument::PercentFtp {
                min: *min,
                max: *max,
            }
        }
        PlannedWorkoutTarget::WattsRange { min, max } => {
            StoredPlannedWorkoutTargetDocument::WattsRange {
                min: *min,
                max: *max,
            }
        }
    }
}

fn map_stored_target_to_domain(target: StoredPlannedWorkoutTargetDocument) -> PlannedWorkoutTarget {
    match target {
        StoredPlannedWorkoutTargetDocument::PercentFtp { min, max } => {
            PlannedWorkoutTarget::PercentFtp { min, max }
        }
        StoredPlannedWorkoutTargetDocument::WattsRange { min, max } => {
            PlannedWorkoutTarget::WattsRange { min, max }
        }
    }
}

fn map_step_kind_to_str(kind: &PlannedWorkoutStepKind) -> &'static str {
    match kind {
        PlannedWorkoutStepKind::Steady => "steady",
        PlannedWorkoutStepKind::Ramp => "ramp",
    }
}

fn map_stored_step_kind(value: &str) -> Result<PlannedWorkoutStepKind, PlannedWorkoutError> {
    match value {
        "steady" => Ok(PlannedWorkoutStepKind::Steady),
        "ramp" => Ok(PlannedWorkoutStepKind::Ramp),
        other => Err(PlannedWorkoutError::Repository(format!(
            "unknown stored planned workout step kind: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_planned_workout_document_round_trip_preserves_fields() {
        let workout = PlannedWorkout::new(
            "planned-imported-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            PlannedWorkoutContent {
                lines: vec![
                    PlannedWorkoutLine::Text(PlannedWorkoutText {
                        text: "Warmup".to_string(),
                    }),
                    PlannedWorkoutLine::Step(PlannedWorkoutStep {
                        duration_seconds: 600,
                        kind: PlannedWorkoutStepKind::Steady,
                        target: PlannedWorkoutTarget::PercentFtp {
                            min: 55.0,
                            max: 65.0,
                        },
                    }),
                ],
            },
        )
        .with_event_metadata(
            Some("Imported Threshold".to_string()),
            Some("Strong over-unders".to_string()),
            Some("Ride".to_string()),
        )
        .with_updated_at(Some(1_700_000_123));

        let mapped =
            map_imported_document_to_domain(map_imported_workout_to_document(&workout)).unwrap();

        assert_eq!(mapped, workout);
    }

    #[test]
    fn imported_planned_workout_document_rejects_unknown_step_kind() {
        let error = map_imported_document_to_domain(ImportedPlannedWorkoutDocument {
            user_id: "user-1".to_string(),
            planned_workout_id: "planned-1".to_string(),
            date: "2026-05-10".to_string(),
            rest_day: false,
            rest_day_reason: None,
            name: None,
            description: None,
            event_type: None,
            workout: StoredPlannedWorkoutContentDocument {
                lines: vec![StoredPlannedWorkoutLineDocument::Step {
                    duration_seconds: 600,
                    step_kind: "mystery".to_string(),
                    target: StoredPlannedWorkoutTargetDocument::WattsRange { min: 180, max: 220 },
                }],
            },
            updated_at_epoch_seconds: None,
        })
        .unwrap_err();

        assert!(matches!(
            error,
            PlannedWorkoutError::Repository(message) if message.contains("mystery")
        ));
    }
}
