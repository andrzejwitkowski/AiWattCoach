use aiwattcoach::{
    adapters::{
        workout_summary_completed_target::CompletedWorkoutTargetAdapter,
        workout_summary_latest_activity::LatestCompletedActivityAdapter,
    },
    domain::{
        completed_workouts::CompletedWorkout,
        workout_summary::{CompletedWorkoutTargetUseCases, LatestCompletedActivityUseCases},
    },
};

#[path = "../support/completed_workouts.rs"]
mod completed_workout_support;

use completed_workout_support::{completed_workout, InMemoryCompletedWorkoutRepository};

fn legacy_completed_workout(
    completed_workout_id: &str,
    start_date_local: &str,
) -> CompletedWorkout {
    completed_workout(completed_workout_id, None, None, None, start_date_local)
}

#[tokio::test]
async fn completed_workout_target_adapter_accepts_legacy_completed_workout_ids() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:legacy-41",
        "2026-03-22T08:00:00",
    )]);
    let adapter = CompletedWorkoutTargetAdapter::new(repository);

    let is_target = adapter
        .is_completed_workout_target("user-1", "legacy-41")
        .await
        .expect("target lookup should succeed");

    assert!(is_target);
}

#[tokio::test]
async fn completed_workout_target_adapter_accepts_canonical_completed_workout_ids() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:legacy-41",
        "2026-03-22T08:00:00",
    )]);
    let adapter = CompletedWorkoutTargetAdapter::new(repository);

    let is_target = adapter
        .is_completed_workout_target("user-1", "intervals-activity:legacy-41")
        .await
        .expect("target lookup should succeed");

    assert!(is_target);
}

#[tokio::test]
async fn completed_workout_target_adapter_returns_cross_source_equivalent_ids_for_same_workout() {
    let repository = completed_workout_repository(vec![
        completed_workout(
            "intervals-activity:i151959404",
            Some("i151959404"),
            Some("training-plan:user-1:source:2026-05-27"),
            Some("459893292"),
            "2026-05-27T15:10:35",
        ),
        completed_workout(
            "wahoo-workout:459893292",
            Some("459893292"),
            Some("training-plan:user-1:source:2026-05-27"),
            Some("459893292"),
            "2026-05-27T13:10:35.000Z",
        ),
    ]);
    let adapter = CompletedWorkoutTargetAdapter::new(repository);

    let resolved = adapter
        .resolve_completed_workout_target("user-1", "i151959404")
        .await
        .expect("target lookup should succeed")
        .expect("completed workout target should resolve");

    assert_eq!(resolved.preferred_workout_id, "i151959404");
    assert_eq!(
        resolved.equivalent_workout_ids,
        vec![
            "i151959404".to_string(),
            "intervals-activity:i151959404".to_string(),
            "459893292".to_string(),
            "wahoo-workout:459893292".to_string(),
        ]
    );
}

#[tokio::test]
async fn latest_completed_activity_adapter_falls_back_to_legacy_completed_workout_id() {
    let repository = completed_workout_repository(vec![legacy_completed_workout(
        "intervals-activity:latest-77",
        "2026-03-22T08:00:00",
    )]);
    let adapter = LatestCompletedActivityAdapter::new(repository);

    let latest_activity_id = adapter
        .latest_completed_activity_id("user-1")
        .await
        .expect("latest lookup should succeed");

    assert_eq!(latest_activity_id.as_deref(), Some("latest-77"));
}

fn completed_workout_repository(
    workouts: Vec<CompletedWorkout>,
) -> InMemoryCompletedWorkoutRepository {
    InMemoryCompletedWorkoutRepository::with_workouts(workouts)
}
