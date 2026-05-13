use std::sync::Arc;

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversationError, CoachConversationMessage, CoachConversationMessageRepository,
        CoachConversationMessageRole, CoachConversationReplyOperation,
        CoachConversationReplyOperationRepository, CoachConversationUseCases,
        CompletedCoachConversationReply, SharedCoachConversationService,
    },
    llm::{LlmError, LlmProvider, LlmTokenUsage},
};

use crate::shared::{
    ConfiguredSettingsService, FixedClock, InMemoryConversationRepository,
    InMemoryMessageRepository, InMemoryReplyOperationRepository, RecordingLlmChatPort,
    RecordingTrainingContextBuilder, StaticLlmConfigProvider, TestIds,
};

#[tokio::test]
async fn calendar_coach_returns_dedicated_error_when_reply_is_already_pending() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(RecordingLlmChatPort::default()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "Need calendar advice".to_string(),
        )
        .await
        .expect("user message should persist");

    reply_operations
        .upsert(CoachConversationReplyOperation::pending(
            "user-1".to_string(),
            conversation.conversation_id.clone(),
            persisted.user_message.id.clone(),
            Some("calendar-coach:user-1:overview".to_string()),
            "message-pending".to_string(),
            1_700_000_000,
        ))
        .await
        .expect("pending operation should persist");

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(error, CoachConversationError::ReplyAlreadyPending);
}

#[tokio::test]
async fn calendar_coach_reuses_completed_operation_without_duplicate_llm_call() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let llm_chat_port = RecordingLlmChatPort::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "Need calendar advice".to_string(),
        )
        .await
        .expect("user message should persist");

    let coach_message = CoachConversationMessage {
        id: "message-completed".to_string(),
        conversation_id: conversation.conversation_id.clone(),
        user_id: "user-1".to_string(),
        role: CoachConversationMessageRole::Coach,
        content: "Recovered calendar reply".to_string(),
        tool_call: None,
        reasoning_content: None,
        created_at_epoch_seconds: 1_700_000_001,
    };
    messages
        .append(coach_message.clone())
        .await
        .expect("coach message should persist");
    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                persisted.user_message.id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                coach_message.id.clone(),
                1_700_000_000,
            )
            .mark_completed(CompletedCoachConversationReply {
                provider: LlmProvider::OpenAi,
                model: "gpt-5".to_string(),
                provider_request_id: Some("req-completed".to_string()),
                reply_message_id: coach_message.id.clone(),
                provider_cache_id: None,
                token_usage: LlmTokenUsage::default(),
                cache_usage: Default::default(),
                updated_at_epoch_seconds: 1_700_000_002,
            }),
        )
        .await
        .expect("completed operation should persist");

    let reply = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .expect("reply should be reused from completed operation");

    assert_eq!(reply.coach_message.id, coach_message.id);
    assert_eq!(reply.coach_message.content, "Recovered calendar reply");
    assert!(llm_chat_port.requests().is_empty());
}

#[tokio::test]
async fn calendar_coach_returns_error_for_previously_failed_operation() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(RecordingLlmChatPort::default()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "Need calendar advice".to_string(),
        )
        .await
        .expect("user message should persist");

    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                persisted.user_message.id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                "message-failed".to_string(),
                1_700_000_000,
            )
            .mark_failed(
                &LlmError::InvalidResponse("broken assistant turn".to_string()),
                1_700_000_001,
            ),
        )
        .await
        .expect("failed operation should persist");

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, CoachConversationError::Llm(_)));
}
