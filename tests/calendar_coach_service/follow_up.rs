use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversation, CoachConversationMessage, CoachConversationMessageRole,
        CoachConversationStatus, CoachConversationUseCases, SharedCoachConversationService,
    },
    llm::{
        LlmCacheUsage, LlmChatMessage, LlmChatResponse, LlmProvider, LlmTokenUsage, LlmToolCall,
    },
};

use crate::shared::{
    ConfiguredSettingsService, FixedClock, InMemoryConversationRepository,
    InMemoryMessageRepository, InMemoryReplyOperationRepository, RecordingTrainingContextBuilder,
    StaticLlmChatPort, StaticLlmConfigProvider, TestIds,
};

#[tokio::test]
async fn calendar_coach_follow_up_replays_last_hidden_assistant_tool_calls() {
    let llm_chat_port = StaticLlmChatPort::new(LlmChatResponse {
        provider: LlmProvider::OpenAi,
        model: "gpt-5".to_string(),
        message: LlmChatMessage::assistant("Coach follow-up"),
        finish_reason: None,
        provider_request_id: Some("req-2".to_string()),
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    });
    let conversations = InMemoryConversationRepository::with_conversation(CoachConversation {
        conversation_id: "conversation-1".to_string(),
        user_id: "user-1".to_string(),
        surface: aiwattcoach::domain::coach_conversation::CoachConversationSurface::Calendar,
        status: CoachConversationStatus::Active,
        focus: aiwattcoach::domain::coach_conversation::CoachConversationFocus::Overview,
        provider_transcript: vec![
            LlmChatMessage::assistant_with_tool_calls(
                "Coach reply",
                vec![LlmToolCall {
                    id: "tool-1".to_string(),
                    name: "lookupCalendar".to_string(),
                    arguments_json: r#"{\"week\":\"2026-W18\"}"#.to_string(),
                }],
            ),
            LlmChatMessage::tool("tool-1", "Calendar lookup result"),
        ],
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    });
    let messages = InMemoryMessageRepository {
        messages: Arc::new(Mutex::new(vec![
            CoachConversationMessage {
                id: "user-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "Need recovery advice".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 1,
            },
            CoachConversationMessage {
                id: "tool-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Tool,
                content: "Tool call: lookupCalendar".to_string(),
                tool_call: Some(aiwattcoach::domain::workout_summary::PublicToolCall {
                    id: "tool-1".to_string(),
                    name: "lookupCalendar".to_string(),
                    arguments_json: r#"{\"week\":\"2026-W18\"}"#.to_string(),
                    arguments_preview: None,
                }),
                reasoning_content: None,
                created_at_epoch_seconds: 2,
            },
            CoachConversationMessage {
                id: "coach-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "Coach reply".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 3,
            },
        ])),
    };

    let service = SharedCoachConversationService::new(
        conversations,
        messages,
        InMemoryReplyOperationRepository::default(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let persisted = service
        .append_calendar_user_message(
            "user-1",
            "conversation-1",
            "What about tomorrow?".to_string(),
        )
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply("user-1", "conversation-1", persisted.user_message.id)
        .await
        .expect("reply should be generated");

    let requests = llm_chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].conversation.len(), 4);
    assert_eq!(
        requests[0].conversation[1].role,
        aiwattcoach::domain::llm::LlmMessageRole::Assistant
    );
    assert_eq!(requests[0].conversation[1].tool_calls.len(), 1);
    assert_eq!(requests[0].conversation[1].tool_calls[0].id, "tool-1");
    assert_eq!(
        requests[0].conversation[2].tool_call_id.as_deref(),
        Some("tool-1")
    );
    assert_eq!(requests[0].conversation[3].content, "What about tomorrow?");
}

#[tokio::test]
async fn calendar_coach_follow_up_replays_multiple_hidden_assistant_turns_with_trimmed_content() {
    let llm_chat_port = StaticLlmChatPort::new(LlmChatResponse {
        provider: LlmProvider::OpenAi,
        model: "gpt-5".to_string(),
        message: LlmChatMessage::assistant("Final answer"),
        finish_reason: None,
        provider_request_id: Some("req-3".to_string()),
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    });
    let conversations = InMemoryConversationRepository::with_conversation(CoachConversation {
        conversation_id: "conversation-1".to_string(),
        user_id: "user-1".to_string(),
        surface: aiwattcoach::domain::coach_conversation::CoachConversationSurface::Calendar,
        status: CoachConversationStatus::Active,
        focus: aiwattcoach::domain::coach_conversation::CoachConversationFocus::Overview,
        provider_transcript: vec![
            LlmChatMessage::assistant_with_tool_calls(
                "First answer\n",
                vec![LlmToolCall {
                    id: "tool-1".to_string(),
                    name: "lookupOne".to_string(),
                    arguments_json: "{}".to_string(),
                }],
            ),
            LlmChatMessage::tool("tool-1", "first result"),
            LlmChatMessage::assistant_with_tool_calls(
                "Second answer\n",
                vec![LlmToolCall {
                    id: "tool-2".to_string(),
                    name: "lookupTwo".to_string(),
                    arguments_json: "{}".to_string(),
                }],
            ),
            LlmChatMessage::tool("tool-2", "second result"),
        ],
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    });
    let messages = InMemoryMessageRepository {
        messages: Arc::new(Mutex::new(vec![
            CoachConversationMessage {
                id: "user-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "First question".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 1,
            },
            CoachConversationMessage {
                id: "coach-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "First answer".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 2,
            },
            CoachConversationMessage {
                id: "user-2".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "Second question".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 3,
            },
            CoachConversationMessage {
                id: "coach-2".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "Second answer".to_string(),
                tool_call: None,
                reasoning_content: None,
                created_at_epoch_seconds: 4,
            },
        ])),
    };

    let service = SharedCoachConversationService::new(
        conversations,
        messages,
        InMemoryReplyOperationRepository::default(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let persisted = service
        .append_calendar_user_message("user-1", "conversation-1", "Third question".to_string())
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply("user-1", "conversation-1", persisted.user_message.id)
        .await
        .expect("reply should be generated");

    let requests = llm_chat_port.requests();
    assert_eq!(requests[0].conversation[1].tool_calls[0].id, "tool-1");
    assert_eq!(
        requests[0].conversation[2].tool_call_id.as_deref(),
        Some("tool-1")
    );
    assert_eq!(requests[0].conversation[4].tool_calls[0].id, "tool-2");
    assert_eq!(
        requests[0].conversation[5].tool_call_id.as_deref(),
        Some("tool-2")
    );
}
