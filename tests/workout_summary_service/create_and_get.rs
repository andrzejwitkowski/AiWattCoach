use aiwattcoach::domain::workout_summary::{WorkoutSummaryError, WorkoutSummaryUseCases};

use crate::shared::{existing_summary, test_service, InMemoryWorkoutSummaryRepository};

#[tokio::test]
async fn create_summary_is_idempotent_when_summary_already_exists() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.create_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.id, "summary-1");
    assert_eq!(summary.workout_id, "workout-1");
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn get_summary_returns_not_found_when_missing() {
    let service = test_service(InMemoryWorkoutSummaryRepository::default());

    let error = service
        .get_summary("user-1", "workout-1")
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::NotFound);
}

#[tokio::test]
async fn update_rpe_rejects_values_outside_expected_range() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let error = service
        .update_rpe("user-1", "workout-1", 11)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation("rpe must be between 1 and 10".to_string())
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_persists_saved_state() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
}

#[tokio::test]
async fn update_rpe_rejects_saved_summary() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let error = service
        .update_rpe("user-1", "workout-1", 8)
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::Locked);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn reopen_summary_clears_saved_state() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let summary = service.reopen_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.saved_at_epoch_seconds, None);
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:None".to_string()]
    );
}

#[tokio::test]
async fn reopen_summary_is_a_no_op_when_already_editable() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.reopen_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.saved_at_epoch_seconds, None);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_requires_rpe() {
    let mut summary = existing_summary();
    summary.rpe = None;
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation(
            "rpe must be set before saving workout summary".to_string()
        )
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}
