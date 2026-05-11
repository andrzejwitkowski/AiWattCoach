mod authoritative;
mod model;
mod ports;
#[cfg(test)]
mod tests;
mod update;

pub use authoritative::AuthoritativePlannedWorkoutRepository;
pub use model::{
    PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
    PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
    PlannedWorkoutText,
};
pub use ports::{BoxFuture, PlannedWorkoutRepository};
pub use update::{
    PlannedWorkoutUpdateService, UpdatePlannedWorkoutCommand, UpdatePlannedWorkoutError,
    UpdatePlannedWorkoutOutcome,
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
