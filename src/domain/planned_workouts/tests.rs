use super::{
    PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutRepository,
    PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
};

#[test]
fn planned_workout_uses_local_canonical_id() {
    let workout = PlannedWorkout::new(
        "planned-1".to_string(),
        "user-1".to_string(),
        "2026-05-01".to_string(),
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
                        max: 55.0,
                    },
                }),
            ],
        },
    );

    assert_eq!(workout.planned_workout_id, "planned-1");
    assert_eq!(workout.user_id, "user-1");
    assert_eq!(workout.date, "2026-05-01");
    assert_eq!(workout.workout.lines.len(), 2);
}

fn assert_planned_workout_repository<T: PlannedWorkoutRepository>() {}

#[test]
fn planned_workout_repository_trait_is_usable() {
    assert_planned_workout_repository::<super::ports::NoopPlannedWorkoutRepository>();
}
