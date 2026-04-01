use std::sync::Arc;

use aiwattcoach::domain::{
    llm::LlmError,
    workout_summary::{MessageRole, WorkoutCoach, WorkoutSummaryUseCases},
};

use crate::shared::{
    default_dev_coach, existing_summary, test_service, test_service_with_coach,
    InMemoryCoachReplyOperationRepository, InMemoryWorkoutSummaryRepository,
};

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
            "claim_pending:workout-1:message-1".to_string(),
            "upsert:workout-1:message-1:Completed".to_string(),
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
    assert_eq!(
        reply_operations.calls(),
        vec![
            format!("claim_pending:workout-1:{}", second.user_message.id),
            format!("upsert:workout-1:{}:Completed", second.user_message.id),
        ]
    );
}

#[derive(Clone)]
struct AlwaysFailingCoach;

impl WorkoutCoach for AlwaysFailingCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &aiwattcoach::domain::workout_summary::WorkoutSummary,
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
