use super::helpers::*;

#[tokio::test]
async fn reclaim_resumes_from_stored_checkpoints_without_regenerating_completed_phases() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_checkpoints(),
        vec![],
        vec![],
        vec![],
        SECOND_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(built.generator.recap_call_count(), 0);
    assert_eq!(built.generator.initial_plan_call_count(), 0);
    assert_eq!(built.generator.correction_call_count(), 0);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert!(operation.validation_issues.is_empty());
}

#[tokio::test]
async fn reclaim_with_stored_recap_skips_redundant_workout_summary_persistence() {
    let call_log = new_call_log();
    let built = build_service_with_operation(
        call_log.clone(),
        stale_pending_operation_with_recap_only(),
        vec![],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        SECOND_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_eq!(built.generator.recap_call_count(), 0);
    assert!(built.workout_summary.persisted_recaps().is_empty());
    assert!(!recorded_calls(&call_log)
        .iter()
        .any(|call| call == "workout_summary.persist_workout_recap"));

    let operation = built.operations.stored_operation();
    let recap_attempts = operation
        .attempts
        .iter()
        .filter(|attempt| attempt.phase == WorkflowPhase::WorkoutRecap)
        .count();
    assert_eq!(recap_attempts, 1);
}

#[tokio::test]
async fn failed_operation_persistence_error_is_surfaced() {
    let operation = TrainingPlanGenerationOperation::pending(
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        USER_ID.to_string(),
        WORKOUT_ID.to_string(),
        date_epoch(FIRST_DAY),
        date_epoch(FIRST_DAY),
    );
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = FailingUpsertTrainingPlanOperationRepository::new(
        operation,
        "operation failure write failed",
    );
    let generator = StubTrainingPlanGenerator::new(
        new_call_log(),
        vec![Err(TrainingPlanError::Validation(
            "recap generation failed".to_string(),
        ))],
        vec![],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations,
        generator,
        StubWorkoutSummaryPort::new(new_call_log()),
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );

    let error = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Repository("operation failure write failed".to_string())
    );
}

#[tokio::test]
async fn fail_operation_preserves_unavailable_error_kind() {
    let built = build_service(
        new_call_log(),
        vec![Err(TrainingPlanError::Unavailable(
            "provider timed out".to_string(),
        ))],
        vec![],
        vec![],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("provider timed out".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
}

#[tokio::test]
async fn heals_pending_operation_when_snapshot_already_exists() {
    let call_log = new_call_log();
    let built = build_service_with_operation(
        call_log,
        stale_pending_operation_with_snapshot_mismatch(),
        vec![],
        vec![],
        vec![],
        FIRST_DAY,
    );

    let snapshot = snapshot_for_first_day();
    built
        .projected_days
        .replace_window(
            snapshot,
            snapshot_projected_days_for_first_day(),
            FIRST_DAY,
            date_epoch(FIRST_DAY),
        )
        .await
        .unwrap();

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(!result.was_generated);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
}

#[tokio::test]
async fn replay_does_not_heal_pending_operation_when_snapshot_exists_without_projected_days() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_snapshot_mismatch(),
        vec![],
        vec![],
        vec![],
        FIRST_DAY,
    );

    built
        .projected_days
        .store_snapshot_only(snapshot_for_first_day());

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation already in progress".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Pending);
    assert!(built.projected_days.stored_days().is_empty());
}
