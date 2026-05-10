use crate::domain::llm::{
    rebuild_conversation_with_provider_transcript, LlmChatMessage, LlmChatResponse, LlmMessageRole,
};

use super::super::{CoachConversation, CoachConversationMessage, CoachConversationMessageRole};

const CALENDAR_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reason about their training from the calendar view. Use the packed training context as factual background. This is a general coaching conversation: the athlete may ask about a workout on a given date, why a planned workout appears in the schedule, how to fuel sessions, how to approach a race strategically, or how the broader week fits together. Be direct, concise, and evidence-based. Do not invent details beyond the provided context and tool results. Do not claim that workouts were regenerated, changed, or committed unless the application explicitly says so. If the athlete asks to regenerate training plans, tell them this is not available from the calendar chat - they need to go to the completed workouts section and save a workout summary to trigger plan generation.";

pub(super) fn final_assistant_text(response: &LlmChatResponse) -> Option<String> {
    crate::domain::llm::final_assistant_text(response)
}

pub(super) fn calendar_coach_system_prompt() -> String {
    format!(
        "{CALENDAR_COACH_SYSTEM_PROMPT_BASE} {}",
        crate::domain::llm::PACKED_TRAINING_CONTEXT_LEGEND,
    )
}

pub(super) fn build_calendar_stable_context(
    conversation: &CoachConversation,
    packed_training_context: &str,
) -> String {
    let mut context = format!(
        "calendar_conversation={{\"conversationId\":\"{}\",\"surface\":\"{}\",\"focus\":\"{}\"}}",
        conversation.conversation_id,
        conversation.surface.as_str(),
        conversation.focus.kind(),
    );

    context.push_str(&format!(
        "\ntraining_context_stable={packed_training_context}"
    ));
    context
}

pub(super) fn build_calendar_volatile_context(
    conversation: &CoachConversation,
    packed_training_context: &str,
) -> String {
    format!(
        "calendar_focus={{\"kind\":\"{}\"}}\ntraining_context_volatile={packed_training_context}",
        conversation.focus.kind(),
    )
}

pub(super) fn build_calendar_conversation(
    messages: &[CoachConversationMessage],
    provider_transcript: &[LlmChatMessage],
    up_to_message_id: &str,
) -> Vec<LlmChatMessage> {
    let messages = match messages.iter().position(|msg| msg.id == up_to_message_id) {
        Some(pos) => &messages[..=pos],
        None => messages,
    };

    let conversation = messages
        .iter()
        .filter_map(|message| match message.role {
            CoachConversationMessageRole::User => Some(LlmChatMessage {
                role: LlmMessageRole::User,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
            }),
            CoachConversationMessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
            }),
            CoachConversationMessageRole::Tool | CoachConversationMessageRole::System => None,
        })
        .collect::<Vec<_>>();

    rebuild_conversation_with_provider_transcript(conversation, provider_transcript)
}
