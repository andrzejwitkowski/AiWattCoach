use aiwattcoach::domain::workout_summary::{MessageRole, WorkoutSummaryUseCases};

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
        .generate_coach_reply(
            "user-1",
            "workout-1",
            persisted.user_message.content.clone(),
        )
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
