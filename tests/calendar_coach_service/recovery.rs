use std::sync::Arc;

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversationMessageRepository, CoachConversationReplyOperation,
        CoachConversationReplyOperationRepository, CoachConversationUseCases,
        PendingCoachConversationReplyCheckpoint, SharedCoachConversationService,
    },
    llm::{LlmChatMessage, LlmError, LlmProvider, LlmTokenUsage, LlmToolCall},
};

use crate::shared::{
    ConfiguredSettingsService, FixedClock, InMemoryConversationRepository,
    InMemoryMessageRepository, InMemoryReplyOperationRepository, RecordingLlmChatPort,
    RecordingTrainingContextBuilder, StaticLlmConfigProvider, TestIds,
};

#[tokio::test]
async fn calendar_coach_marks_tool_only_recovery_as_failed() {
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
            "Need recovery advice".to_string(),
        )
        .await
        .expect("user message should persist");
    let user_message_id = persisted.user_message.id.clone();

    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                user_message_id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                "message-tool-only".to_string(),
                1_699_999_000,
            )
            .record_provider_response(PendingCoachConversationReplyCheckpoint {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini".to_string(),
                provider_request_id: Some("req-tool-only".to_string()),
                provider_cache_id: None,
                token_usage: LlmTokenUsage::default(),
                cache_usage: Default::default(),
                provider_transcript: vec![LlmChatMessage::assistant_with_tool_calls(
                    "",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: "{}".to_string(),
                    }],
                )],
                finish_reason: None,
                updated_at_epoch_seconds: 1_699_999_001,
            }),
        )
        .await
        .unwrap();

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            user_message_id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::coach_conversation::CoachConversationError::Llm(
            LlmError::InvalidResponse("assistant reply missing final text message".to_string())
        )
    );
    let stored = reply_operations
        .find_by_user_message_id("user-1", &conversation.conversation_id, &user_message_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.status,
        aiwattcoach::domain::coach_conversation::CoachConversationReplyOperationStatus::Failed
    );
    assert_eq!(stored.public_tool_call_ids, vec!["tool-1".to_string()]);
}

#[tokio::test]
async fn calendar_coach_recovery_persists_materialized_tool_call_ids_before_completion() {
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
            "Need recovery advice".to_string(),
        )
        .await
        .expect("user message should persist");
    let user_message_id = persisted.user_message.id.clone();

    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                user_message_id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                "message-recovered".to_string(),
                1_699_999_000,
            )
            .record_provider_response(PendingCoachConversationReplyCheckpoint {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini".to_string(),
                provider_request_id: Some("req-recovery".to_string()),
                provider_cache_id: None,
                token_usage: LlmTokenUsage::default(),
                cache_usage: Default::default(),
                provider_transcript: vec![LlmChatMessage::assistant_with_tool_calls(
                    "Recovered coach reply",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: "{}".to_string(),
                    }],
                )],
                finish_reason: None,
                updated_at_epoch_seconds: 1_699_999_001,
            }),
        )
        .await
        .expect("recovery operation should persist");

    let reply = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            user_message_id.clone(),
        )
        .await
        .expect("reply should recover from persisted transcript");

    let stored = reply_operations
        .find_by_user_message_id("user-1", &conversation.conversation_id, &user_message_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        stored.status,
        aiwattcoach::domain::coach_conversation::CoachConversationReplyOperationStatus::Completed
    );
    assert_eq!(stored.public_tool_call_ids, vec!["tool-1".to_string()]);
    assert_eq!(reply.coach_message.id, "message-recovered");
    let stored_messages = messages
        .list_by_user_id_and_conversation_id("user-1", &conversation.conversation_id)
        .await
        .expect("messages should list");
    let stored_ids = stored_messages
        .into_iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    assert_eq!(
        stored_ids,
        vec![
            user_message_id,
            "tool-1".to_string(),
            "message-recovered".to_string()
        ]
    );
}

#[tokio::test]
async fn calendar_coach_marks_fresh_tool_only_response_as_failed() {
    let llm_chat_port =
        crate::shared::StaticLlmChatPort::new(aiwattcoach::domain::llm::LlmChatResponse {
            provider: LlmProvider::OpenRouter,
            model: "openai/gpt-4o-mini".to_string(),
            message: LlmChatMessage::assistant_with_tool_calls(
                "",
                vec![LlmToolCall {
                    id: "tool-1".to_string(),
                    name: "lookupCalendar".to_string(),
                    arguments_json: "{}".to_string(),
                }],
            ),
            finish_reason: None,
            provider_request_id: Some("req-tool-only-fresh".to_string()),
            usage: LlmTokenUsage::default(),
            cache: Default::default(),
        });
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(llm_chat_port),
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
            "Need recovery advice".to_string(),
        )
        .await
        .expect("user message should persist");

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::coach_conversation::CoachConversationError::Llm(
            LlmError::InvalidResponse("assistant reply missing final text message".to_string())
        )
    );
    let stored = reply_operations
        .find_by_user_message_id(
            "user-1",
            &conversation.conversation_id,
            &persisted.user_message.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.status,
        aiwattcoach::domain::coach_conversation::CoachConversationReplyOperationStatus::Failed
    );
}
