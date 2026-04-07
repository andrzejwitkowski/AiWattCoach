use super::support::*;

#[tokio::test]
async fn generates_snapshot_and_projected_days_for_saved_workout() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator.clone(),
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(result.snapshot.days.len(), 14);
    assert_eq!(result.active_projected_days.len(), 13);
    assert_eq!(snapshots.stored_snapshots().len(), 1);
    assert_eq!(
        projected_days
            .stored_days()
            .iter()
            .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > FIRST_DAY)
            .count(),
        13
    );
    assert!(!projected_days.stored_days().iter().any(|day| {
        day.date == FIRST_DAY
            && day.superseded_at_epoch_seconds.is_none()
            && day.date.as_str() > FIRST_DAY
    }));

    let operation = operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert_eq!(
        operation.operation_key,
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        )
    );
}

#[tokio::test]
async fn persists_workout_recap_before_generating_training_plan_window() {
    let call_log = new_call_log();
    let service = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    service
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "workout_summary.persist_workout_recap",
        "generator.generate_initial_plan_window",
    );
}

#[tokio::test]
async fn checkpoints_recap_in_operation_before_persisting_to_workout_summary() {
    let call_log = new_call_log();
    let built = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "operation.upsert",
        "workout_summary.persist_workout_recap",
    );
}

#[tokio::test]
async fn replay_of_same_saved_workout_generation_is_idempotent() {
    let call_log = new_call_log();
    let built = build_service(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    let first = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();
    let replay = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(first.was_generated);
    assert!(!replay.was_generated);
    assert_eq!(first.snapshot.operation_key, replay.snapshot.operation_key);
    assert_eq!(built.generator.recap_call_count(), 1);
    assert_eq!(built.generator.initial_plan_call_count(), 1);
    assert_eq!(built.snapshots.stored_snapshots().len(), 1);
    assert_eq!(built.projected_days.stored_days().len(), 14);
}

#[tokio::test]
async fn existing_pending_operation_returns_unavailable_without_calling_generator() {
    let operation = TrainingPlanGenerationOperation::pending(
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        USER_ID.to_string(),
        WORKOUT_ID.to_string(),
        date_epoch(FIRST_DAY),
        date_epoch(SECOND_DAY),
    );
    let built = build_service_with_operation(
        new_call_log(),
        operation,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![Ok(single_rest_day("2026-04-10"))],
        SECOND_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation already in progress".to_string())
    );
    assert_eq!(built.generator.recap_call_count(), 0);
    assert_eq!(built.generator.initial_plan_call_count(), 0);
    assert_eq!(built.generator.correction_call_count(), 0);
}

#[tokio::test]
async fn next_day_generation_supersedes_only_overlapping_future_projected_days() {
    let first = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );
    first
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let second_generator = StubTrainingPlanGenerator::new(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(SECOND_DAY))],
        vec![],
    );
    let second_service = TrainingPlanGenerationService::new(
        first.snapshots.clone(),
        first.projected_days.clone(),
        first.operations.clone(),
        second_generator,
        first.workout_summary.clone(),
        FixedClock {
            now_epoch_seconds: date_epoch(SECOND_DAY),
        },
    );

    second_service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(SECOND_DAY))
        .await
        .unwrap();

    let stored_days = first.projected_days.stored_days();
    let active_days = stored_days
        .iter()
        .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > SECOND_DAY)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(active_days.len(), 13);
    assert!(!active_days.iter().any(|day| day.date == FIRST_DAY));
    assert!(!active_days.iter().any(|day| day.date == SECOND_DAY));
    assert!(stored_days.iter().any(|day| {
        day.date == SECOND_DAY
            && day.operation_key
                == format!(
                    "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
                    date_epoch(FIRST_DAY)
                )
            && day.superseded_at_epoch_seconds.is_some()
    }));
}

#[tokio::test]
async fn successful_generation_records_real_workflow_attempts() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(single_rest_day("2026-04-10"))],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let operation = built.operations.stored_operation();
    let phases = operation
        .attempts
        .iter()
        .map(|attempt| attempt.phase.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            WorkflowPhase::WorkoutRecap,
            WorkflowPhase::InitialGeneration,
            WorkflowPhase::Correction,
            WorkflowPhase::ProjectionUpdate,
        ]
    );
}
