use aiwattcoach::domain::workout_summary::{
    WorkoutRecap, WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryUseCases,
};

use crate::shared::{
    existing_summary, test_service, test_service_with_training_plan,
    InMemoryWorkoutSummaryRepository, RecordingTrainingPlanService,
};

#[tokio::test]
async fn create_summary_is_idempotent_when_summary_already_exists() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.create_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.id, "summary-1");
    assert_eq!(summary.workout_id, "workout-1");
    assert_eq!(summary.workout_recap_text, None);
    assert_eq!(summary.workout_recap_provider, None);
    assert_eq!(summary.workout_recap_model, None);
    assert_eq!(summary.workout_recap_generated_at_epoch_seconds, None);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn create_summary_defaults_recap_fields_to_none() {
    let repository = InMemoryWorkoutSummaryRepository::default();
    let service = test_service(repository);

    let summary = service.create_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.workout_recap_text, None);
    assert_eq!(summary.workout_recap_provider, None);
    assert_eq!(summary.workout_recap_model, None);
    assert_eq!(summary.workout_recap_generated_at_epoch_seconds, None);
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
async fn mark_saved_triggers_training_plan_generation_after_persisting_saved_state() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let training_plan = RecordingTrainingPlanService::default();
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Repository("training plan result not seeded in test".to_string())
    );
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
    assert_eq!(
        training_plan.calls(),
        vec!["generate_for_saved_workout:user-1:workout-1:1700000000".to_string()]
    );
    assert_eq!(
        repository
            .find_by_user_id_and_workout_id("user-1", "workout-1")
            .await
            .unwrap()
            .unwrap()
            .saved_at_epoch_seconds,
        Some(1_700_000_000)
    );
}

#[tokio::test]
async fn repeat_mark_saved_retries_training_plan_generation_for_already_saved_summary() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let training_plan = RecordingTrainingPlanService::default();
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Repository("training plan result not seeded in test".to_string())
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
    assert_eq!(
        training_plan.calls(),
        vec!["generate_for_saved_workout:user-1:workout-1:1700000000".to_string()]
    );
}

#[tokio::test]
async fn mark_saved_maps_training_plan_failure_to_repository_error_after_persisting_save() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let training_plan = RecordingTrainingPlanService::default();
    training_plan.fail_next(
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "llm temporarily unavailable".to_string(),
        ),
    );
    let service =
        test_service_with_training_plan(repository.clone(), std::sync::Arc::new(training_plan));

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Repository("llm temporarily unavailable".to_string())
    );
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
    assert_eq!(
        repository
            .find_by_user_id_and_workout_id("user-1", "workout-1")
            .await
            .unwrap()
            .unwrap()
            .saved_at_epoch_seconds,
        Some(1_700_000_000)
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
async fn persist_workout_recap_updates_recap_fields_and_timestamp() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Strong finish after a rough middle block.",
                "openai",
                "gpt-5.4-mini",
                1_700_000_123,
            ),
        )
        .await
        .unwrap();

    assert_eq!(
        summary.workout_recap_text,
        Some("Strong finish after a rough middle block.".to_string())
    );
    assert_eq!(summary.workout_recap_provider, Some("openai".to_string()));
    assert_eq!(
        summary.workout_recap_model,
        Some("gpt-5.4-mini".to_string())
    );
    assert_eq!(
        summary.workout_recap_generated_at_epoch_seconds,
        Some(1_700_000_123)
    );
    assert_eq!(summary.updated_at_epoch_seconds, 1_700_000_000);
    assert_eq!(
        repository.calls(),
        vec!["persist_workout_recap:workout-1".to_string()]
    );
}

#[tokio::test]
async fn persist_workout_recap_is_idempotent_for_repeated_values() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Legs came around late and cadence stayed smooth.",
                "openrouter",
                "google/gemini-3-flash-preview",
                1_700_000_321,
            ),
        )
        .await
        .unwrap();

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Legs came around late and cadence stayed smooth.",
                "openrouter",
                "google/gemini-3-flash-preview",
                1_700_000_321,
            ),
        )
        .await
        .unwrap();

    assert_eq!(
        summary.workout_recap_text,
        Some("Legs came around late and cadence stayed smooth.".to_string())
    );
    assert_eq!(
        summary.workout_recap_provider,
        Some("openrouter".to_string())
    );
    assert_eq!(
        summary.workout_recap_model,
        Some("google/gemini-3-flash-preview".to_string())
    );
    assert_eq!(
        summary.workout_recap_generated_at_epoch_seconds,
        Some(1_700_000_321)
    );
    assert_eq!(summary.updated_at_epoch_seconds, 1_700_000_000);
    assert_eq!(
        repository.calls(),
        vec!["persist_workout_recap:workout-1".to_string()]
    );
}

#[tokio::test]
async fn persist_workout_recap_does_not_bump_updated_at_when_recap_is_already_stored() {
    let mut summary = existing_summary();
    summary.workout_recap_text = Some("Strong close after a controlled opener.".to_string());
    summary.workout_recap_provider = Some("openai".to_string());
    summary.workout_recap_model = Some("gpt-5.4-mini".to_string());
    summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_123);
    summary.updated_at_epoch_seconds = 1_699_999_999;

    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Strong close after a controlled opener.",
                "openai",
                "gpt-5.4-mini",
                1_700_000_123,
            ),
        )
        .await
        .unwrap();

    assert_eq!(summary.updated_at_epoch_seconds, 1_699_999_999);
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
