use serde::{Deserialize, Serialize};

use crate::domain::{
    intervals::{
        PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutRepeat, PlannedWorkoutStep,
        PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    training_plan::TrainingPlanError,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct PlannedWorkoutDocument {
    lines: Vec<PlannedWorkoutLineDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
enum PlannedWorkoutLineDocument {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "repeat")]
    Repeat { title: Option<String>, count: i64 },
    #[serde(rename = "step")]
    Step {
        duration_seconds: i32,
        step_kind: String,
        percent_min: Option<f64>,
        percent_max: Option<f64>,
        watts_min: Option<i32>,
        watts_max: Option<i32>,
    },
}

pub(crate) fn map_planned_workout_to_document(
    workout: &PlannedWorkout,
) -> Result<PlannedWorkoutDocument, TrainingPlanError> {
    Ok(PlannedWorkoutDocument {
        lines: workout
            .lines
            .iter()
            .map(map_line_to_document)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn map_document_to_planned_workout(
    document: PlannedWorkoutDocument,
) -> Result<PlannedWorkout, TrainingPlanError> {
    Ok(PlannedWorkout {
        lines: document
            .lines
            .into_iter()
            .map(map_document_to_line)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn map_line_to_document(
    line: &PlannedWorkoutLine,
) -> Result<PlannedWorkoutLineDocument, TrainingPlanError> {
    match line {
        PlannedWorkoutLine::Text(PlannedWorkoutText { text }) => {
            Ok(PlannedWorkoutLineDocument::Text { text: text.clone() })
        }
        PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat { title, count }) => {
            Ok(PlannedWorkoutLineDocument::Repeat {
                title: title.clone(),
                count: i64::try_from(*count).map_err(|_| {
                    TrainingPlanError::Repository(
                        "planned workout repeat count exceeds Mongo integer range".to_string(),
                    )
                })?,
            })
        }
        PlannedWorkoutLine::Step(PlannedWorkoutStep {
            duration_seconds,
            kind,
            target,
        }) => {
            let (percent_min, percent_max, watts_min, watts_max) = match target {
                PlannedWorkoutTarget::PercentFtp { min, max } => {
                    (Some(*min), Some(*max), None, None)
                }
                PlannedWorkoutTarget::WattsRange { min, max } => {
                    (None, None, Some(*min), Some(*max))
                }
            };
            Ok(PlannedWorkoutLineDocument::Step {
                duration_seconds: *duration_seconds,
                step_kind: match kind {
                    PlannedWorkoutStepKind::Steady => "steady",
                    PlannedWorkoutStepKind::Ramp => "ramp",
                }
                .to_string(),
                percent_min,
                percent_max,
                watts_min,
                watts_max,
            })
        }
    }
}

fn map_document_to_line(
    document: PlannedWorkoutLineDocument,
) -> Result<PlannedWorkoutLine, TrainingPlanError> {
    match document {
        PlannedWorkoutLineDocument::Text { text } => {
            Ok(PlannedWorkoutLine::Text(PlannedWorkoutText { text }))
        }
        PlannedWorkoutLineDocument::Repeat { title, count } => {
            Ok(PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat {
                title,
                count: usize::try_from(count).map_err(|_| {
                    TrainingPlanError::Repository(
                        "invalid planned workout repeat count in Mongo document".to_string(),
                    )
                })?,
            }))
        }
        PlannedWorkoutLineDocument::Step {
            duration_seconds,
            step_kind,
            percent_min,
            percent_max,
            watts_min,
            watts_max,
        } => Ok(PlannedWorkoutLine::Step(PlannedWorkoutStep {
            duration_seconds,
            kind: match step_kind.as_str() {
                "steady" => PlannedWorkoutStepKind::Steady,
                "ramp" => PlannedWorkoutStepKind::Ramp,
                other => {
                    return Err(TrainingPlanError::Repository(format!(
                        "unknown planned workout step kind: {other}"
                    )))
                }
            },
            target: match (percent_min, percent_max, watts_min, watts_max) {
                (Some(min), Some(max), None, None) => PlannedWorkoutTarget::PercentFtp { min, max },
                (None, None, Some(min), Some(max)) => PlannedWorkoutTarget::WattsRange { min, max },
                _ => {
                    return Err(TrainingPlanError::Repository(
                        "invalid planned workout target in Mongo document".to_string(),
                    ))
                }
            },
        })),
    }
}
