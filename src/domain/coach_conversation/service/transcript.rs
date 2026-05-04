use crate::domain::llm::{LlmChatMessage, LlmChatResponse, LlmMessageRole};

use super::super::{CoachConversation, CoachConversationMessage, CoachConversationMessageRole};

const CALENDAR_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reason about their training from the calendar view. Use the packed training context as factual background. This is a general coaching conversation: the athlete may ask about a workout on a given date, why a planned workout appears in the schedule, how to fuel sessions, how to approach a race strategically, or how the broader week fits together. Be direct, concise, and evidence-based. Do not invent details beyond the provided context. Do not claim that workouts were regenerated, changed, or committed unless the application explicitly says so. If the athlete asks to regenerate training plans, tell them this is not available from the calendar chat - they need to go to the completed workouts section and save a workout summary to trigger plan generation.";

pub(super) fn final_assistant_text(response: &LlmChatResponse) -> Option<String> {
    response
        .assistant_text()
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string)
}

pub(super) fn merge_hidden_transcript_entries(
    mut existing: Vec<LlmChatMessage>,
    pending: &[LlmChatMessage],
) -> Vec<LlmChatMessage> {
    let max_overlap = existing.len().min(pending.len());
    let overlap = (1..=max_overlap)
        .rev()
        .find(|overlap| existing[existing.len() - overlap..] == pending[..*overlap])
        .unwrap_or(0);

    for entry in &pending[overlap..] {
        existing.push(entry.clone());
    }

    existing
}

fn hidden_tool_messages_for_assistant(
    hidden_transcript: &[LlmChatMessage],
    assistant: &LlmChatMessage,
) -> Vec<LlmChatMessage> {
    assistant
        .tool_calls
        .iter()
        .filter_map(|tool_call| {
            hidden_transcript
                .iter()
                .find(|message| {
                    message.role == LlmMessageRole::Tool
                        && message.tool_call_id.as_deref() == Some(tool_call.id.as_str())
                })
                .cloned()
        })
        .collect()
}

fn rebuild_conversation_with_hidden_transcript(
    conversation: Vec<LlmChatMessage>,
    hidden_transcript: &[LlmChatMessage],
) -> Vec<LlmChatMessage> {
    let hidden_assistants = hidden_transcript
        .iter()
        .filter(|message| message.role == LlmMessageRole::Assistant)
        .cloned()
        .collect::<Vec<_>>();
    let mut hidden_assistant_index = 0;
    let mut rebuilt = Vec::with_capacity(conversation.len() + hidden_transcript.len());

    for message in conversation {
        if message.role != LlmMessageRole::Assistant {
            rebuilt.push(message);
            continue;
        }

        let assistant = hidden_assistants
            .get(hidden_assistant_index)
            .cloned()
            .unwrap_or(message);
        hidden_assistant_index += 1;
        rebuilt.push(assistant.clone());
        rebuilt.extend(hidden_tool_messages_for_assistant(
            hidden_transcript,
            &assistant,
        ));
    }

    rebuilt
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
    hidden_transcript: &[LlmChatMessage],
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
            }),
            CoachConversationMessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }),
            CoachConversationMessageRole::Tool | CoachConversationMessageRole::System => None,
        })
        .collect::<Vec<_>>();

    rebuild_conversation_with_hidden_transcript(conversation, hidden_transcript)
}

#[cfg(test)]
mod tests {
    use crate::domain::llm::LlmChatMessage;

    use super::merge_hidden_transcript_entries;

    #[test]
    fn merge_hidden_transcript_entries_preserves_repeated_identical_messages() {
        let repeated = LlmChatMessage::assistant("same tool result");

        let merged = merge_hidden_transcript_entries(
            vec![LlmChatMessage::assistant("earlier reply"), repeated.clone()],
            &[repeated.clone(), repeated.clone()],
        );

        assert_eq!(
            merged,
            vec![
                LlmChatMessage::assistant("earlier reply"),
                repeated.clone(),
                repeated,
            ]
        );
    }
}
