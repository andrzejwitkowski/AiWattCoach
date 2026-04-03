use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    llm::{BoxFuture, LlmCacheUsage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
    workout_summary::{
        CoachReplyOperation, MessageRole, PendingCoachReplyCheckpoint, WorkoutCoach,
        WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryUseCases,
    },
};

use crate::shared::{
    default_dev_coach, existing_summary, test_service, test_service_with_coach,
    InMemoryCoachReplyOperationRepository, InMemoryWorkoutSummaryRepository,
};

#[derive(Clone, Default)]
struct CountingCoach {
    calls: Arc<Mutex<Vec<String>>>,
}

impl CountingCoach {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl WorkoutCoach for CountingCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        self.calls.lock().unwrap().push(user_message.to_string());
        let message = format!("Coach reply to: {user_message}");
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "counting-coach".to_string(),
                message,
                provider_request_id: Some("counting-req-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            })
        })
    }
}

#[tokio::test]
async fn send_message_persists_user_message_before_coach_reply() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let turn = service
        .send_message("user-1", "workout-1", "Legs felt heavy today".to_string())
        .await
        .unwrap();

    assert_eq!(turn.user_message.role, MessageRole::User);
    assert_eq!(turn.coach_message.role, MessageRole::Coach);
    assert_eq!(turn.summary.messages.len(), 2);
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
}

#[tokio::test]
async fn append_user_message_persists_only_user_message() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Legs felt heavy today".to_string())
        .await
        .unwrap();

    assert_eq!(persisted.user_message.role, MessageRole::User);
    assert_eq!(persisted.summary.messages.len(), 1);
    assert_eq!(
        repository.calls(),
        vec!["append_message:workout-1:user".to_string()]
    );
}

#[tokio::test]
async fn generate_coach_reply_persists_pending_operation_before_coach_message() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository.clone(),
        reply_operations.clone(),
        default_dev_coach(),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
        ]
    );
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_targets_the_persisted_message_id_when_contents_repeat() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository.clone(),
        reply_operations.clone(),
        default_dev_coach(),
    );

    let _first = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();
    let second = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let reply = service
        .generate_coach_reply("user-1", "workout-1", second.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", second.user_message.id),
            format!("upsert:workout-1:{}:Pending", second.user_message.id),
            format!("upsert:workout-1:{}:Completed", second.user_message.id),
        ]
    );
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );

    assert!(reply
        .summary
        .messages
        .iter()
        .any(|message| message.id == second.user_message.id));
}

#[derive(Clone)]
struct AlwaysFailingCoach;

impl WorkoutCoach for AlwaysFailingCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        _user_message: &str,
    ) -> aiwattcoach::domain::llm::BoxFuture<
        Result<aiwattcoach::domain::llm::LlmChatResponse, aiwattcoach::domain::llm::LlmError>,
    > {
        Box::pin(async move { Err(LlmError::RateLimited("provider throttled".to_string())) })
    }
}

#[derive(Clone)]
struct OneTimeFailureCoach {
    error: LlmError,
    calls: Arc<Mutex<usize>>,
}

impl OneTimeFailureCoach {
    fn new(error: LlmError) -> Self {
        Self {
            error,
            calls: Arc::new(Mutex::new(0)),
        }
    }
}

impl WorkoutCoach for OneTimeFailureCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
    ) -> aiwattcoach::domain::llm::BoxFuture<
        Result<aiwattcoach::domain::llm::LlmChatResponse, aiwattcoach::domain::llm::LlmError>,
    > {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        let call_number = *calls;
        let error = self.error.clone();
        let user_message = user_message.to_string();
        Box::pin(async move {
            if call_number == 1 {
                Err(error)
            } else {
                Ok(LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "recovered-coach".to_string(),
                    message: format!("Recovered reply to: {user_message}"),
                    provider_request_id: Some("recovered-req".to_string()),
                    usage: LlmTokenUsage::default(),
                    cache: LlmCacheUsage::default(),
                })
            }
        })
    }
}

#[tokio::test]
async fn generate_coach_reply_preserves_structured_llm_errors() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository,
        reply_operations.clone(),
        Arc::new(AlwaysFailingCoach),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::RateLimited(
            "provider throttled".to_string()
        ))
    );
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Failed", persisted.user_message.id),
        ]
    );
}

#[derive(Clone)]
struct OversizedContextCoach;

impl WorkoutCoach for OversizedContextCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        _user_message: &str,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        Box::pin(async move {
            Err(LlmError::ContextTooLarge(
                "packed training context exceeds model limits".to_string(),
            ))
        })
    }
}

#[tokio::test]
async fn generate_coach_reply_preserves_oversized_context_error() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository,
        reply_operations.clone(),
        Arc::new(OversizedContextCoach),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::ContextTooLarge(
            "packed training context exceeds model limits".to_string()
        ))
    );
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Failed", persisted.user_message.id),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_returns_dedicated_error_when_reply_is_already_pending() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service =
        test_service_with_coach(repository, reply_operations.clone(), default_dev_coach());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.seed(CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        persisted.user_message.id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "message-pending".to_string(),
        1_700_000_000,
    ));

    let error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::ReplyAlreadyPending);
}

#[tokio::test]
async fn generate_coach_reply_retries_after_failed_operation() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository.clone(),
        reply_operations.clone(),
        default_dev_coach(),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.seed(
        CoachReplyOperation::pending(
            "user-1".to_string(),
            "workout-1".to_string(),
            persisted.user_message.id.clone(),
            Some("workout-summary:user-1:workout-1".to_string()),
            "message-retry".to_string(),
            1_700_000_000,
        )
        .mark_failed(
            &LlmError::RateLimited("provider throttled".to_string()),
            1_700_000_001,
        ),
    );

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("seed:workout-1:{}:Failed", persisted.user_message.id),
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
        ]
    );
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_retries_after_non_retryable_failure() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(OneTimeFailureCoach::new(LlmError::ProviderRejected(
        "invalid model".to_string(),
    )));
    let service = test_service_with_coach(repository.clone(), reply_operations.clone(), coach);

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let first_error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap_err();
    assert_eq!(
        first_error,
        aiwattcoach::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::ProviderRejected(
            "invalid model".to_string()
        ))
    );

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert!(reply.coach_message.content.contains("Recovered reply to"));
}

#[tokio::test]
async fn generate_coach_reply_reuses_completed_operation_without_duplicate_coach_call() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service =
        test_service_with_coach(repository.clone(), reply_operations.clone(), coach.clone());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let first_reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();
    let second_reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(coach.calls(), vec!["Need feedback".to_string()]);
    assert_eq!(first_reply.coach_message.id, second_reply.coach_message.id);
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
        ]
    );
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_recovers_stale_pending_operation() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let service = test_service_with_coach(
        repository.clone(),
        reply_operations.clone(),
        default_dev_coach(),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.seed(CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        persisted.user_message.id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "message-stale".to_string(),
        1_699_999_000,
    ));

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.id, "message-stale");
    assert_eq!(reply.coach_message.role, MessageRole::Coach);
}

#[tokio::test]
async fn generate_coach_reply_recovers_existing_message_without_losing_provider_metadata() {
    let mut summary = existing_summary();
    let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
        id: "message-user-1".to_string(),
        role: MessageRole::User,
        content: "Need feedback".to_string(),
        created_at_epoch_seconds: 1_699_999_000,
    };
    let coach_message = aiwattcoach::domain::workout_summary::ConversationMessage {
        id: "message-coach-1".to_string(),
        role: MessageRole::Coach,
        content: "Recovered coach reply".to_string(),
        created_at_epoch_seconds: 1_699_999_001,
    };
    summary.messages = vec![user_message.clone(), coach_message.clone()];

    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service = test_service_with_coach(repository, reply_operations.clone(), coach.clone());

    let stale_operation = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        user_message.id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        coach_message.id.clone(),
        1_699_999_000,
    )
    .record_provider_response(PendingCoachReplyCheckpoint {
        provider: LlmProvider::Gemini,
        model: "gemini-2.5-flash".to_string(),
        provider_request_id: Some("req-123".to_string()),
        provider_cache_id: Some("cachedContents/cache-1".to_string()),
        token_usage: LlmTokenUsage {
            input_tokens: Some(111),
            output_tokens: Some(22),
            total_tokens: Some(133),
        },
        cache_usage: LlmCacheUsage {
            provider_cache_id: Some("cachedContents/cache-1".to_string()),
            provider_cache_key: None,
            cache_hit: true,
            cached_read_tokens: Some(80),
            cache_write_tokens: Some(0),
            cache_expires_at_epoch_seconds: Some(1_700_010_000),
            cache_discount: Some("0.0012".to_string()),
        },
        response_message: "Recovered coach reply".to_string(),
        updated_at_epoch_seconds: 1_699_999_000,
    });
    reply_operations.seed(stale_operation);

    let reply = service
        .generate_coach_reply("user-1", "workout-1", user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(coach.calls(), Vec::<String>::new());
    assert_eq!(reply.coach_message, coach_message);

    let stored = reply_operations
        .get("user-1", "workout-1", &user_message.id)
        .unwrap();
    assert_eq!(
        stored.status,
        aiwattcoach::domain::workout_summary::CoachReplyOperationStatus::Completed
    );
    assert_eq!(stored.provider, Some(LlmProvider::Gemini));
    assert_eq!(stored.model.as_deref(), Some("gemini-2.5-flash"));
    assert_eq!(stored.provider_request_id.as_deref(), Some("req-123"));
    assert_eq!(
        stored.provider_cache_id.as_deref(),
        Some("cachedContents/cache-1")
    );
    assert_eq!(
        stored
            .token_usage
            .as_ref()
            .and_then(|usage| usage.total_tokens),
        Some(133)
    );
    assert_eq!(
        stored
            .cache_usage
            .as_ref()
            .and_then(|cache| cache.cached_read_tokens),
        Some(80)
    );
}

#[tokio::test]
async fn generate_coach_reply_replays_persisted_response_message_after_partial_crash() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service =
        test_service_with_coach(repository.clone(), reply_operations.clone(), coach.clone());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    let partial = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        persisted.user_message.id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "message-partial".to_string(),
        1_699_999_000,
    )
    .record_provider_response(PendingCoachReplyCheckpoint {
        provider: LlmProvider::OpenAi,
        model: "gpt-4o-mini".to_string(),
        provider_request_id: Some("req-partial".to_string()),
        provider_cache_id: None,
        token_usage: LlmTokenUsage::default(),
        cache_usage: LlmCacheUsage::default(),
        response_message: "Persisted before crash".to_string(),
        updated_at_epoch_seconds: 1_699_999_001,
    });
    reply_operations.seed(partial);

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(coach.calls(), Vec::<String>::new());
    assert_eq!(reply.coach_message.id, "message-partial");
    assert_eq!(reply.coach_message.content, "Persisted before crash");
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_retries_completion_write_after_coach_message_append() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service =
        test_service_with_coach(repository.clone(), reply_operations.clone(), coach.clone());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.fail_next_completed_upsert("completion write failed");

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(coach.calls(), vec!["Need feedback".to_string()]);
    assert_eq!(
        repository.calls(),
        vec![
            "append_message:workout-1:user".to_string(),
            "append_message:workout-1:coach".to_string(),
        ]
    );
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_retries_success_checkpoint_write_before_returning() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service =
        test_service_with_coach(repository.clone(), reply_operations.clone(), coach.clone());

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.fail_next_pending_upsert("pending checkpoint write failed");

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(coach.calls(), vec!["Need feedback".to_string()]);
    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Pending", persisted.user_message.id),
            format!("upsert:workout-1:{}:Completed", persisted.user_message.id),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_retries_failure_checkpoint_write_before_returning() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(AlwaysFailingCoach);
    let service = test_service_with_coach(repository, reply_operations.clone(), coach);

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    reply_operations.fail_next_failed_upsert("failed checkpoint write failed");

    let error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::RateLimited(
            "provider throttled".to_string()
        ))
    );
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", persisted.user_message.id),
            format!("upsert:workout-1:{}:Failed", persisted.user_message.id),
            format!("upsert:workout-1:{}:Failed", persisted.user_message.id),
        ]
    );
}

#[tokio::test]
async fn generate_coach_reply_replays_persisted_response_for_saved_summary() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_050);

    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let coach = Arc::new(CountingCoach::default());
    let service =
        test_service_with_coach(repository.clone(), reply_operations.clone(), coach.clone());

    let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
        id: "message-user-saved".to_string(),
        role: MessageRole::User,
        content: "Need feedback".to_string(),
        created_at_epoch_seconds: 1_699_999_000,
    };
    repository
        .append_message("user-1", "workout-1", user_message.clone(), 1_699_999_000)
        .await
        .unwrap();

    let partial = CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        user_message.id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "message-saved-replay".to_string(),
        1_699_999_000,
    )
    .record_provider_response(PendingCoachReplyCheckpoint {
        provider: LlmProvider::OpenAi,
        model: "gpt-4o-mini".to_string(),
        provider_request_id: Some("req-saved".to_string()),
        provider_cache_id: None,
        token_usage: LlmTokenUsage::default(),
        cache_usage: LlmCacheUsage::default(),
        response_message: "Recovered even though summary was saved".to_string(),
        updated_at_epoch_seconds: 1_699_999_001,
    });
    reply_operations.seed(partial);

    let reply = service
        .generate_coach_reply("user-1", "workout-1", user_message.id.clone())
        .await
        .unwrap();

    assert_eq!(coach.calls(), Vec::<String>::new());
    assert_eq!(reply.coach_message.id, "message-saved-replay");
    assert_eq!(
        reply.coach_message.content,
        "Recovered even though summary was saved"
    );
}
