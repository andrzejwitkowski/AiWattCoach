use std::sync::Arc;

use aiwattcoach::domain::coach_conversation::{
    CoachConversationMessageRole, CoachConversationUseCases, SharedCoachConversationService,
};

use crate::shared::{
    ConfiguredSettingsService, FixedClock, InMemoryConversationRepository,
    InMemoryMessageRepository, InMemoryReplyOperationRepository, NoopContextCacheRepository,
    RecordingLlmChatPort, RecordingTrainingContextBuilder, StaticLlmConfigProvider, TestIds,
};

#[tokio::test]
async fn calendar_coach_generate_reply_uses_calendar_overview_context_and_no_summary_text() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let llm_chat_port = RecordingLlmChatPort::default();
    let training_context_builder = RecordingTrainingContextBuilder::default();

    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations,
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(training_context_builder.clone()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService))
    .with_context_cache_repository(Arc::new(NoopContextCacheRepository));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "What should I do today?".to_string(),
        )
        .await
        .expect("user message should persist");

    assert!(!persisted.athlete_summary_may_regenerate_before_reply);

    let reply = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .expect("reply should be generated");

    assert!(!reply.athlete_summary_was_regenerated);
    assert_eq!(training_context_builder.build_calls(), Vec::<String>::new());
    assert_eq!(
        training_context_builder.calendar_overview_calls(),
        vec!["user-1".to_string()]
    );
    assert_eq!(
        training_context_builder.athlete_summary_calls(),
        Vec::<String>::new()
    );

    let requests = llm_chat_port.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(request
        .stable_context
        .contains("training_context_stable={\"source\":\"summary\"}"));
    assert!(!request.stable_context.contains("athlete_summary_text="));
    assert!(request
        .volatile_context
        .contains("training_context_volatile={\"focus\":\"summary\"}"));
    assert_eq!(request.conversation.len(), 1);
    assert_eq!(request.conversation[0].content, "What should I do today?");
    assert_eq!(
        request.cache_scope_key.as_deref(),
        Some("calendar-coach:user-1:overview")
    );
    assert_eq!(
        reply.coach_message.role,
        CoachConversationMessageRole::Coach
    );
    assert_eq!(reply.coach_message.content, "Coach reply");
}

#[tokio::test]
async fn calendar_coach_send_message_result_keeps_summary_regeneration_hint_false() {
    let service = SharedCoachConversationService::new(
        InMemoryConversationRepository::default(),
        InMemoryMessageRepository::default(),
        InMemoryReplyOperationRepository::default(),
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
            "Check tomorrow".to_string(),
        )
        .await
        .expect("user message should persist");

    assert!(!persisted.athlete_summary_may_regenerate_before_reply);
}
