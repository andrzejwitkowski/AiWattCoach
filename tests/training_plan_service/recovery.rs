use super::support::*;
use aiwattcoach::domain::{
    llm::{LlmChatMessage, LlmChatResponse, LlmFinishReason, LlmProvider, LlmTokenUsage},
    llm_tools::LlmToolLoopOutput,
    training_plan::BoxFuture,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct CheckpointingInitialPlanGenerator {
    restored_states: Arc<Mutex<Vec<Option<LlmToolLoopState>>>>,
    initial_calls: Arc<Mutex<u32>>,
}

impl CheckpointingInitialPlanGenerator {
    fn restored_states(&self) -> Vec<Option<LlmToolLoopState>> {
        self.restored_states.lock().unwrap().clone()
    }
}

#[derive(Clone, Default)]
struct CompletedResponseCrashGenerator {
    restored_states: Arc<Mutex<Vec<Option<LlmToolLoopState>>>>,
    generator_calls: Arc<Mutex<u32>>,
    provider_calls: Arc<Mutex<u32>>,
}

impl CompletedResponseCrashGenerator {
    fn restored_states(&self) -> Vec<Option<LlmToolLoopState>> {
        self.restored_states.lock().unwrap().clone()
    }

    fn provider_call_count(&self) -> u32 {
        *self.provider_calls.lock().unwrap()
    }
}

impl TrainingPlanGenerator for CheckpointingInitialPlanGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        Box::pin(async move { unreachable!("recovery test uses stored recap") })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        self.restored_states.lock().unwrap().push(restored_state);
        let call_number = {
            let mut initial_calls = self.initial_calls.lock().unwrap();
            *initial_calls += 1;
            *initial_calls
        };

        Box::pin(async move {
            if call_number == 1 {
                checkpoint.expect("expected initial checkpoint callback")(LlmToolLoopState {
                    round_count: 2,
                    ..Default::default()
                })
                .await?;
                return Err(TrainingPlanError::Unavailable(
                    "simulated crash after initial tool round".to_string(),
                ));
            }

            Ok(TrainingPlanPhaseOutput {
                raw_response: valid_plan_window(FIRST_DAY),
                description: None,
                tool_loop_state: LlmToolLoopState {
                    round_count: 3,
                    ..Default::default()
                },
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _invalid_day_sections: &str,
        _issues: Vec<ValidationIssue>,
        _restored_state: Option<LlmToolLoopState>,
        _checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        Box::pin(async move { unreachable!("recovery test does not use correction flow") })
    }
}

impl TrainingPlanGenerator for CompletedResponseCrashGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        Box::pin(async move { unreachable!("recovery test uses stored recap") })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        self.restored_states
            .lock()
            .unwrap()
            .push(restored_state.clone());
        let call_number = {
            let mut calls = self.generator_calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        let provider_calls = self.provider_calls.clone();

        Box::pin(async move {
            if restored_state
                .as_ref()
                .and_then(|state| state.completed_response.as_ref())
                .is_none()
            {
                let mut provider_calls = provider_calls.lock().unwrap();
                *provider_calls += 1;
            }

            if call_number == 1 {
                checkpoint.expect("expected initial checkpoint callback")(
                    LlmToolLoopOutput::from_response(LlmChatResponse {
                        provider: LlmProvider::Gemini,
                        model: "gemini-3.1-pro".to_string(),
                        message: LlmChatMessage::assistant(valid_plan_window(FIRST_DAY)),
                        finish_reason: Some(LlmFinishReason::Stop),
                        provider_request_id: Some("req-completed".to_string()),
                        usage: LlmTokenUsage::default(),
                        cache: Default::default(),
                    })
                    .state,
                )
                .await?;
                return Err(TrainingPlanError::Unavailable(
                    "simulated crash after final no-tool response".to_string(),
                ));
            }

            Ok(TrainingPlanPhaseOutput {
                raw_response: restored_state
                    .as_ref()
                    .and_then(|state| state.completed_response.as_ref())
                    .map(|response| response.message.content.clone())
                    .unwrap_or_else(|| {
                        "2026-04-06\nRest Day: fallback provider response".to_string()
                    }),
                description: None,
                tool_loop_state: restored_state.unwrap_or(LlmToolLoopState {
                    round_count: 2,
                    ..Default::default()
                }),
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _invalid_day_sections: &str,
        _issues: Vec<ValidationIssue>,
        _restored_state: Option<LlmToolLoopState>,
        _checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        Box::pin(async move { unreachable!("recovery test does not use correction flow") })
    }
}

#[tokio::test]
async fn reclaim_reuses_completed_initial_tool_loop_state_without_second_provider_call() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::with_operation(
        call_log.clone(),
        stale_pending_operation_with_recap_only(),
    );
    let generator = CompletedResponseCrashGenerator::default();
    let workout_summary = StubWorkoutSummaryPort::new(call_log);
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations.clone(),
        generator.clone(),
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(SECOND_DAY),
        },
    );

    let error = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("simulated crash after final no-tool response".to_string())
    );

    let failed_operation = operations.stored_operation();
    assert_eq!(failed_operation.status, WorkflowStatus::Failed);
    assert!(failed_operation.raw_plan_response.is_none());
    assert_eq!(
        failed_operation
            .initial_plan_tool_loop_state
            .as_ref()
            .map(|state| state.round_count),
        Some(1)
    );
    assert!(failed_operation
        .initial_plan_tool_loop_state
        .as_ref()
        .and_then(|state| state.completed_response.as_ref())
        .is_some());

    service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let restored = generator.restored_states();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0], None);
    assert_eq!(restored[1].as_ref().map(|state| state.round_count), Some(1));
    assert!(restored[1]
        .as_ref()
        .and_then(|state| state.completed_response.as_ref())
        .is_some());
    assert_eq!(generator.provider_call_count(), 1);

    let operation = operations.stored_operation();
    assert_eq!(
        operation.raw_plan_response.as_deref(),
        Some(valid_plan_window(FIRST_DAY).as_str())
    );
}

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
    assert_eq!(
        built.generator.initial_restored_states(),
        Vec::<Option<LlmToolLoopState>>::new()
    );
    assert_eq!(
        built.generator.correction_restored_states(),
        Vec::<Option<LlmToolLoopState>>::new()
    );
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
    assert_eq!(built.generator.initial_restored_states(), vec![None]);
}

#[tokio::test]
async fn reclaim_reuses_persisted_initial_tool_loop_state_before_raw_response_exists() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::with_operation(
        call_log.clone(),
        stale_pending_operation_with_recap_only(),
    );
    let generator = CheckpointingInitialPlanGenerator::default();
    let workout_summary = StubWorkoutSummaryPort::new(call_log);
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations.clone(),
        generator.clone(),
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(SECOND_DAY),
        },
    );

    let error = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("simulated crash after initial tool round".to_string())
    );

    let failed_operation = operations.stored_operation();
    assert_eq!(failed_operation.status, WorkflowStatus::Failed);
    assert!(failed_operation.raw_plan_response.is_none());
    assert_eq!(
        failed_operation
            .initial_plan_tool_loop_state
            .as_ref()
            .map(|state| state.round_count),
        Some(2)
    );

    service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let restored = generator.restored_states();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0], None);
    assert_eq!(restored[1].as_ref().map(|state| state.round_count), Some(2));
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
