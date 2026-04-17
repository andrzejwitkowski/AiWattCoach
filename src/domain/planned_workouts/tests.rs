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
    assert_eq!(workout.name, None);
    assert_eq!(workout.description, None);
    assert_eq!(workout.event_type, None);
    assert_eq!(workout.workout.lines.len(), 2);
}

#[test]
fn planned_workout_can_store_reader_metadata() {
    let workout = PlannedWorkout::new(
        "planned-1".to_string(),
        "user-1".to_string(),
        "2026-05-01".to_string(),
        PlannedWorkoutContent { lines: Vec::new() },
    )
    .with_event_metadata(
        Some("Threshold builder".to_string()),
        Some("Strong over-unders".to_string()),
        Some("Ride".to_string()),
    );

    assert_eq!(workout.name.as_deref(), Some("Threshold builder"));
    assert_eq!(workout.description.as_deref(), Some("Strong over-unders"));
    assert_eq!(workout.event_type.as_deref(), Some("Ride"));
}

#[test]
fn planned_workout_can_represent_rest_day() {
    let workout = PlannedWorkout::new(
        "planned-1".to_string(),
        "user-1".to_string(),
        "2026-05-01".to_string(),
        PlannedWorkoutContent { lines: Vec::new() },
    )
    .as_rest_day(Some(
        "Need recovery before next quality session".to_string(),
    ));

    assert!(workout.rest_day);
    assert_eq!(
        workout.rest_day_reason.as_deref(),
        Some("Need recovery before next quality session")
    );
}

fn assert_planned_workout_repository<T: PlannedWorkoutRepository>() {}

#[test]
fn planned_workout_repository_trait_is_usable() {
    assert_planned_workout_repository::<super::ports::NoopPlannedWorkoutRepository>();
}

#[tokio::test]
async fn planned_workout_repository_lists_by_user_and_date_range() {
    let repository = super::ports::NoopPlannedWorkoutRepository::default();
    repository
        .upsert(sample_workout("planned-2", "user-1", "2026-05-02"))
        .await
        .unwrap();
    repository
        .upsert(sample_workout("planned-1", "user-1", "2026-05-01"))
        .await
        .unwrap();
    repository
        .upsert(sample_workout("planned-3", "user-2", "2026-05-01"))
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(workouts.len(), 2);
    assert_eq!(workouts[0].planned_workout_id, "planned-1");
    assert_eq!(workouts[1].planned_workout_id, "planned-2");
}

fn sample_workout(planned_workout_id: &str, user_id: &str, date: &str) -> PlannedWorkout {
    PlannedWorkout::new(
        planned_workout_id.to_string(),
        user_id.to_string(),
        date.to_string(),
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
    )
}
