use std::sync::Arc;

use aiwattcoach::domain::coach_conversation::{
    CoachConversationRepository, CoachConversationUseCases, SharedCoachConversationService,
};
use aiwattcoach::domain::llm::LlmChatMessage;

use crate::shared::{
    ConfiguredSettingsService, FixedClock, InMemoryConversationRepository,
    InMemoryMessageRepository, InMemoryReplyOperationRepository, RecordingLlmChatPort,
    RecordingTrainingContextBuilder, StaticLlmConfigProvider, TestIds,
};

#[tokio::test]
async fn calendar_coach_retries_provider_transcript_write_after_compare_and_set_conflict() {
    let llm_chat_port = RecordingLlmChatPort::default();
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages,
        reply_operations,
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

    {
        let mut guard = conversations.conversation.lock().unwrap();
        guard.as_mut().unwrap().provider_transcript = vec![LlmChatMessage::assistant("Turn 0")];
    }
    conversations.conflict_next_hidden_transcript_write();

    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "What should I do tomorrow?".to_string(),
        )
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id,
        )
        .await
        .expect("reply should be generated");

    let stored = conversations
        .find_by_user_id_and_conversation_id("user-1", &conversation.conversation_id)
        .await
        .unwrap()
        .unwrap();
    let provider_contents = stored
        .provider_transcript
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>();

    assert!(provider_contents.contains(&"Turn 0"));
    assert!(provider_contents.contains(&"Concurrent calendar update"));
    assert!(provider_contents.contains(&"Coach reply"));
}
