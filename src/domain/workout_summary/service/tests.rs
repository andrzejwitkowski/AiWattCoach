use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{LlmChatMessage, LlmError},
    workout_summary::{
        CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository,
        ConversationMessage, MessageRole, MockWorkoutCoach, PublicToolCall, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryService,
    },
};

#[derive(Clone)]
pub(crate) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(crate) struct FixedIds;

impl IdGenerator for FixedIds {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

#[test]
fn existing_llm_failure_to_error_falls_back_to_internal_when_kind_is_missing() {
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
        service.existing_llm_failure_to_error(operation),
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

#[tokio::test]
async fn materialize_public_tool_messages_records_existing_message_without_duplicate_append() {
    let service = WorkoutSummaryService::with_coach(
        ExistingMessageSummaryRepository::with_messages(vec![ConversationMessage {
            id: "tool-1".to_string(),
            role: MessageRole::Tool,
            content: "Tool call: existing".to_string(),
            tool_call: Some(PublicToolCall {
                id: "tool-1".to_string(),
                name: "existing".to_string(),
                arguments_json: "{}".to_string(),
                arguments_preview: None,
            }),
            questions: Vec::new(),
            created_at_epoch_seconds: 1_700_000_000,
        }]),
        StubReplyOperations,
        FixedClock,
        FixedIds,
        Arc::new(MockWorkoutCoach),
    );
    let operation = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        "message-1".to_string(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "coach-message-1".to_string(),
        1_700_000_000,
    );

    let updated = service
        .materialize_public_tool_messages(
            "user-1",
            "workout-1",
            operation,
            &[PublicToolCall {
                id: "tool-1".to_string(),
                name: "existing".to_string(),
                arguments_json: "{}".to_string(),
                arguments_preview: None,
            }],
        )
        .await
        .expect("existing tool message should not fail");

    assert_eq!(updated.public_tool_call_ids, vec!["tool-1".to_string()]);
}

#[tokio::test]
async fn materialize_public_tool_messages_appends_only_missing_calls_in_order() {
    let repository = ExistingMessageSummaryRepository::with_messages(vec![ConversationMessage {
        id: "tool-2".to_string(),
        role: MessageRole::Tool,
        content: "Tool call: second".to_string(),
        tool_call: Some(PublicToolCall {
            id: "tool-2".to_string(),
            name: "second".to_string(),
            arguments_json: "{}".to_string(),
            arguments_preview: None,
        }),
        questions: Vec::new(),
        created_at_epoch_seconds: 1_700_000_000,
    }]);
    let service = WorkoutSummaryService::with_coach(
        repository.clone(),
        StubReplyOperations,
        FixedClock,
        FixedIds,
        Arc::new(MockWorkoutCoach),
    );
    let operation = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        "message-1".to_string(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "coach-message-1".to_string(),
        1_700_000_000,
    );
    let public_tool_calls = vec![
        PublicToolCall {
            id: "tool-1".to_string(),
            name: "first".to_string(),
            arguments_json: "{}".to_string(),
            arguments_preview: None,
        },
        PublicToolCall {
            id: "tool-2".to_string(),
            name: "second".to_string(),
            arguments_json: "{}".to_string(),
            arguments_preview: None,
        },
        PublicToolCall {
            id: "tool-3".to_string(),
            name: "third".to_string(),
            arguments_json: "{}".to_string(),
            arguments_preview: None,
        },
    ];

    let updated = service
        .materialize_public_tool_messages("user-1", "workout-1", operation, &public_tool_calls)
        .await
        .expect("tool materialization should succeed");

    assert_eq!(
        updated.public_tool_call_ids,
        vec![
            "tool-1".to_string(),
            "tool-2".to_string(),
            "tool-3".to_string()
        ]
    );
    assert_eq!(
        repository.appended_message_ids(),
        vec!["tool-1".to_string(), "tool-3".to_string()]
    );
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

    fn replace_provider_transcript(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _provider_transcript: Vec<crate::domain::llm::LlmChatMessage>,
        _expected_updated_at_epoch_seconds: i64,
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

#[derive(Clone, Default)]
pub(crate) struct ExistingMessageSummaryRepository {
    messages: Arc<std::sync::Mutex<Vec<ConversationMessage>>>,
    appended_message_ids: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ExistingMessageSummaryRepository {
    pub(crate) fn with_messages(messages: Vec<ConversationMessage>) -> Self {
        Self {
            messages: Arc::new(std::sync::Mutex::new(messages)),
            appended_message_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn appended_message_ids(&self) -> Vec<String> {
        self.appended_message_ids
            .lock()
            .expect("lock should succeed")
            .clone()
    }
}

impl WorkoutSummaryRepository for ExistingMessageSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> super::BoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let messages = self.messages.lock().expect("lock should succeed").clone();
        Box::pin(async move {
            Ok(Some(WorkoutSummary {
                id: "summary-1".to_string(),
                user_id,
                workout_id,
                rpe: Some(5),
                messages,
                provider_transcript: Vec::new(),
                saved_at_epoch_seconds: None,
                workout_recap_text: None,
                workout_recap_provider: None,
                workout_recap_model: None,
                workout_recap_generated_at_epoch_seconds: None,
                created_at_epoch_seconds: 1_700_000_000,
                updated_at_epoch_seconds: 1_700_000_000,
            }))
        })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        _workout_ids: Vec<String>,
    ) -> super::BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn create(
        &self,
        _summary: WorkoutSummary,
    ) -> super::BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
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
        message: ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
        let messages = self.messages.clone();
        let appended_message_ids = self.appended_message_ids.clone();
        Box::pin(async move {
            appended_message_ids
                .lock()
                .expect("lock should succeed")
                .push(message.id.clone());
            messages.lock().expect("lock should succeed").push(message);
            Ok(())
        })
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

    fn replace_provider_transcript(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _provider_transcript: Vec<LlmChatMessage>,
        _expected_updated_at_epoch_seconds: i64,
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
        message_id: &str,
    ) -> super::BoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let message_id = message_id.to_string();
        let messages = self.messages.lock().expect("lock should succeed").clone();
        Box::pin(async move {
            Ok(messages
                .into_iter()
                .find(|message| message.id == message_id))
        })
    }
}

#[derive(Clone)]
pub(crate) struct StubReplyOperations;

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
    ) -> super::BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        Box::pin(async { Err(WorkoutSummaryError::NotFound) })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> super::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        Box::pin(async move { Ok(operation) })
    }
}

#[derive(Clone, Default)]
pub(crate) struct RecordingReplyOperations {
    operation: Arc<std::sync::Mutex<Option<CoachReplyOperation>>>,
}

impl RecordingReplyOperations {
    pub(crate) fn last_upserted_operation(&self) -> Option<CoachReplyOperation> {
        self.operation.lock().expect("lock should succeed").clone()
    }
}

impl CoachReplyOperationRepository for RecordingReplyOperations {
    fn find_by_user_message_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _user_message_id: &str,
    ) -> super::BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let operation = self.operation.lock().expect("lock should succeed").clone();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        _operation: CoachReplyOperation,
        _stale_before_epoch_seconds: i64,
    ) -> super::BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        Box::pin(async { Err(WorkoutSummaryError::NotFound) })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> super::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let state = self.operation.clone();
        Box::pin(async move {
            *state.lock().expect("lock should succeed") = Some(operation.clone());
            Ok(operation)
        })
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
    ) -> super::BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
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
