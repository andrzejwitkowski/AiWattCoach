use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    llm::{BoxFuture, LlmCacheUsage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
    workout_summary::{
        CoachReplyOperation, MessageRole, WorkoutCoach, WorkoutSummary, WorkoutSummaryError,
        WorkoutSummaryUseCases,
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
            1_700_000_000,
        )
        .mark_failed("provider throttled".to_string(), 1_700_000_001),
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
