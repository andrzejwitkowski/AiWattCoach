use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::LlmError,
    workout_summary::{
        CoachReplyOperation, CoachReplyOperationRepository, MockWorkoutCoach, WorkoutSummaryError,
        WorkoutSummaryRepository, WorkoutSummaryService,
    },
};

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
struct FixedIds;

impl IdGenerator for FixedIds {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

#[test]
fn map_existing_llm_failure_falls_back_to_internal_error_when_kind_is_missing() {
    let service = WorkoutSummaryService::with_coach(
        StubSummaryRepository,
        StubReplyOperations,
        FixedClock,
        FixedIds,
        Arc::new(MockWorkoutCoach),
    );

    let mut operation = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        "message-1".to_string(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "coach-message-1".to_string(),
        1_700_000_000,
    )
    .mark_failed(
        &LlmError::Internal("persisted failure without kind".to_string()),
        1_700_000_001,
    );
    operation.failure_kind = None;

    assert_eq!(
        service.map_existing_llm_failure(operation),
        WorkoutSummaryError::Llm(LlmError::Internal(
            "persisted failure without kind".to_string()
        ))
    );
}

#[tokio::test]
async fn persist_post_provider_operation_does_not_retry_non_repository_errors() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let service = WorkoutSummaryService::with_coach(
        StubSummaryRepository,
        NonRepositoryFailingReplyOperations {
            attempts: attempts.clone(),
        },
        FixedClock,
        FixedIds,
        Arc::new(MockWorkoutCoach),
    );

    let error = service
        .persist_post_provider_operation(
            CoachReplyOperation::pending(
                "user-1".to_string(),
                "workout-1".to_string(),
                "message-1".to_string(),
                Some("workout-summary:user-1:workout-1".to_string()),
                "coach-message-1".to_string(),
                1_700_000_000,
            ),
            "persist_success_checkpoint",
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation("semantic failure".to_string())
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[derive(Clone)]
struct StubSummaryRepository;

impl WorkoutSummaryRepository for StubSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> super::BoxFuture<Result<Option<super::WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        _workout_ids: Vec<String>,
    ) -> super::BoxFuture<Result<Vec<super::WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn create(
        &self,
        _summary: super::WorkoutSummary,
    ) -> super::BoxFuture<Result<super::WorkoutSummary, WorkoutSummaryError>> {
        Box::pin(async { Err(WorkoutSummaryError::NotFound) })
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
        _updated_at_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
        Box::pin(async { Ok(()) })
    }

    fn append_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message: super::ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
        Box::pin(async { Ok(()) })
    }

    fn set_saved_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: Option<i64>,
        _updated_at_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
        Box::pin(async { Ok(()) })
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: super::WorkoutRecap,
        _updated_at_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
        Box::pin(async { Ok(()) })
    }

    fn find_message_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message_id: &str,
    ) -> super::BoxFuture<Result<Option<super::ConversationMessage>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }
}

#[derive(Clone)]
struct StubReplyOperations;

impl CoachReplyOperationRepository for StubReplyOperations {
    fn find_by_user_message_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _user_message_id: &str,
    ) -> super::BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn claim_pending(
        &self,
        _operation: CoachReplyOperation,
        _stale_before_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<super::CoachReplyClaimResult, WorkoutSummaryError>> {
        Box::pin(async { Err(WorkoutSummaryError::NotFound) })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> super::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        Box::pin(async move { Ok(operation) })
    }
}

#[derive(Clone)]
struct NonRepositoryFailingReplyOperations {
    attempts: Arc<AtomicUsize>,
}

impl CoachReplyOperationRepository for NonRepositoryFailingReplyOperations {
    fn find_by_user_message_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _user_message_id: &str,
    ) -> super::BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn claim_pending(
        &self,
        _operation: CoachReplyOperation,
        _stale_before_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<super::CoachReplyClaimResult, WorkoutSummaryError>> {
        Box::pin(async { Err(WorkoutSummaryError::NotFound) })
    }

    fn upsert(
        &self,
        _operation: CoachReplyOperation,
    ) -> super::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let attempts = self.attempts.clone();
        Box::pin(async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(WorkoutSummaryError::Validation(
                "semantic failure".to_string(),
            ))
        })
    }
}
