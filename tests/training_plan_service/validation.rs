use super::helpers::*;

#[tokio::test]
async fn invalid_day_parse_records_date_scoped_validation_issue() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(
        operation.validation_issues[0],
        ValidationIssue {
            scope: "2026-04-10".to_string(),
            message: "failed to parse day 2026-04-10: invalid planned workout step: - nope"
                .to_string(),
        }
    );
}

#[tokio::test]
async fn correction_round_merges_corrected_days_and_succeeds() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(single_rest_day("2026-04-10"))],
        FIRST_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(built.generator.correction_call_count(), 1);
    let correction_inputs = built.generator.correction_inputs();
    assert_eq!(correction_inputs.len(), 1);
    assert_eq!(
        correction_inputs[0].0,
        "2026-04-10\nBroken session\n- nope".to_string()
    );
    assert_eq!(correction_inputs[0].1.len(), 1);
    let corrected_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-10")
        .expect("expected corrected day");
    assert!(corrected_day.rest_day);
    let untouched_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-09")
        .expect("expected untouched day");
    assert!(!untouched_day.rest_day);
    assert_eq!(result.snapshot.days.len(), 14);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert!(operation.validation_issues.is_empty());
}

#[tokio::test]
async fn correction_retry_exhaustion_marks_failed_operation_and_keeps_raw_responses() {
    let initial_raw = plan_with_invalid_day(FIRST_DAY, "2026-04-10");
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(initial_raw.clone())],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.raw_plan_response.as_deref(),
        Some(initial_raw.as_str())
    );
    assert!(operation.raw_correction_response.is_some());
    assert_eq!(built.generator.correction_call_count(), 2);
}

#[tokio::test]
async fn reclaim_with_stored_invalid_correction_response_keeps_full_retry_budget() {
    let built = build_service_with_operation(
        new_call_log(),
        stale_pending_operation_with_invalid_correction_response(),
        vec![],
        vec![],
        vec![
            Ok(single_invalid_day("2026-04-10")),
            Ok(single_invalid_day("2026-04-10")),
        ],
        SECOND_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );
    assert_eq!(built.generator.correction_call_count(), 2);

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(operation.validation_issues[0].scope, "2026-04-10");
}

#[tokio::test]
async fn correction_retry_persists_latest_invalid_scope_after_scope_shifts() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-11")),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-11")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(operation.validation_issues[0].scope, "2026-04-11");

    let correction_inputs = built.generator.correction_inputs();
    assert_eq!(correction_inputs.len(), 2);
    assert_eq!(correction_inputs[0].1[0].scope, "2026-04-10");
    assert_eq!(correction_inputs[1].1[0].scope, "2026-04-11");
}

#[tokio::test]
async fn correction_retry_keeps_omitted_invalid_day_in_retry_set() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![
            Ok("2026-04-11\nRest Day".to_string()),
            Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        ],
        FIRST_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation failed validation".to_string())
    );

    let correction_inputs = built.generator.correction_inputs();
    assert_eq!(correction_inputs.len(), 2);
    assert_eq!(correction_inputs[0].1[0].scope, "2026-04-10");
    assert_eq!(correction_inputs[1].1[0].scope, "2026-04-10");

    let operation = built.operations.stored_operation();
    assert_eq!(operation.validation_issues.len(), 1);
    assert_eq!(operation.validation_issues[0].scope, "2026-04-10");
}

#[tokio::test]
async fn correction_ignores_changes_for_dates_that_were_not_invalid() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(
            "2026-04-10\nRest Day\n\n2026-04-09\nRest Day".to_string()
        )],
        FIRST_DAY,
    );

    let result = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let corrected_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-10")
        .expect("expected corrected day");
    assert!(corrected_day.rest_day);

    let untouched_day = result
        .snapshot
        .days
        .iter()
        .find(|day| day.date == "2026-04-09")
        .expect("expected untouched day");
    assert!(!untouched_day.rest_day);
}

#[tokio::test]
async fn duplicate_dates_fail_durably_instead_of_overwriting() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(window_with_duplicate_date(FIRST_DAY, "2026-04-10"))],
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
        TrainingPlanError::Validation("duplicate planned workout day: 2026-04-10".to_string())
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.failure.unwrap().phase,
        WorkflowPhase::InitialGeneration
    );
}

#[tokio::test]
async fn non_contiguous_windows_fail_durably() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(window_with_gap(FIRST_DAY, "2026-04-12"))],
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
        TrainingPlanError::Validation(
            "training plan window must contain exactly 14 contiguous dated days".to_string()
        )
    );

    let operation = built.operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Failed);
    assert_eq!(
        operation.failure.unwrap().phase,
        WorkflowPhase::InitialGeneration
    );
}
