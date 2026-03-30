use aiwattcoach::domain::workout_summary::{WorkoutSummaryError, WorkoutSummaryUseCases};

use crate::shared::{existing_summary, test_service, InMemoryWorkoutSummaryRepository};

#[tokio::test]
async fn create_summary_is_idempotent_when_summary_already_exists() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.create_summary("user-1", "event-1").await.unwrap();

    assert_eq!(summary.id, "summary-1");
    assert_eq!(summary.event_id, "event-1");
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn get_summary_returns_not_found_when_missing() {
    let service = test_service(InMemoryWorkoutSummaryRepository::default());

    let error = service.get_summary("user-1", "event-1").await.unwrap_err();

    assert_eq!(error, WorkoutSummaryError::NotFound);
}

#[tokio::test]
async fn update_rpe_rejects_values_outside_expected_range() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let error = service
        .update_rpe("user-1", "event-1", 11)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation("rpe must be between 1 and 10".to_string())
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}
