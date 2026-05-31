use super::{LlmChatMessage, LlmChatResponse, LlmMessageRole};

pub(crate) fn timestamped_message_content(content: &str, created_at_epoch_seconds: i64) -> String {
    format!(
        "[sent_at={}]\n{}",
        super::epoch_seconds_to_rfc3339(created_at_epoch_seconds),
        content
    )
}

pub(crate) fn final_assistant_text(response: &LlmChatResponse) -> Option<String> {
    response
        .assistant_text()
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string)
}

pub(crate) fn last_nonempty_assistant_content(
    provider_transcript: &[LlmChatMessage],
) -> Option<String> {
    provider_transcript
        .iter()
        .rev()
        .find(|message| message.role == LlmMessageRole::Assistant)
        .map(|message| message.content.clone())
        .filter(|content| !content.trim().is_empty())
}

pub(crate) fn merge_provider_transcript_entries(
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

pub(crate) fn rebuild_conversation_with_provider_transcript(
    conversation: Vec<LlmChatMessage>,
    provider_transcript: &[LlmChatMessage],
) -> Vec<LlmChatMessage> {
    let provider_assistants = provider_transcript
        .iter()
        .filter(|message| message.role == LlmMessageRole::Assistant)
        .cloned()
        .collect::<Vec<_>>();
    let mut provider_assistant_index = 0;
    let mut rebuilt = Vec::with_capacity(conversation.len() + provider_transcript.len());

    for message in conversation {
        if message.role != LlmMessageRole::Assistant {
            rebuilt.push(message);
            continue;
        }

        let assistant = provider_assistants
            .get(provider_assistant_index)
            .cloned()
            .unwrap_or(message);
        provider_assistant_index += 1;
        rebuilt.push(assistant.clone());
        rebuilt.extend(provider_tool_messages_for_assistant(
            provider_transcript,
            &assistant,
        ));
    }

    rebuilt
}

pub(crate) fn next_provider_transcript_updated_at_epoch_seconds(
    expected_updated_at_epoch_seconds: i64,
    now_epoch_seconds: i64,
) -> i64 {
    now_epoch_seconds.max(expected_updated_at_epoch_seconds.saturating_add(1))
}

pub(crate) fn provider_transcript_from_legacy_response(
    provider_transcript: Vec<LlmChatMessage>,
    legacy_response_message: Option<String>,
) -> Vec<LlmChatMessage> {
    if provider_transcript.is_empty() {
        legacy_response_message
            .map(LlmChatMessage::assistant)
            .into_iter()
            .collect()
    } else {
        provider_transcript
    }
}

fn provider_tool_messages_for_assistant(
    provider_transcript: &[LlmChatMessage],
    assistant: &LlmChatMessage,
) -> Vec<LlmChatMessage> {
    assistant
        .tool_calls
        .iter()
        .filter_map(|tool_call| {
            provider_transcript
                .iter()
                .find(|message| {
                    message.role == LlmMessageRole::Tool
                        && message.tool_call_id.as_deref() == Some(tool_call.id.as_str())
                })
                .cloned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        merge_provider_transcript_entries, next_provider_transcript_updated_at_epoch_seconds,
        provider_transcript_from_legacy_response, rebuild_conversation_with_provider_transcript,
        timestamped_message_content,
    };
    use crate::domain::llm::{LlmChatMessage, LlmMessageRole, LlmToolCall};

    #[test]
    fn merge_provider_transcript_entries_preserves_repeated_identical_messages() {
        let repeated = LlmChatMessage::assistant("same tool result");

        let merged = merge_provider_transcript_entries(
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

    #[test]
    fn provider_transcript_from_legacy_response_reuses_legacy_checkpoint() {
        assert_eq!(
            provider_transcript_from_legacy_response(
                Vec::new(),
                Some("Legacy checkpoint".to_string())
            ),
            vec![LlmChatMessage::assistant("Legacy checkpoint")]
        );
    }

    #[test]
    fn rebuild_conversation_with_provider_transcript_replays_assistant_tool_messages() {
        let rebuilt = rebuild_conversation_with_provider_transcript(
            vec![
                LlmChatMessage::user("Need feedback"),
                LlmChatMessage::assistant("Coach reply"),
            ],
            &[
                LlmChatMessage::assistant_with_tool_calls(
                    "Coach reply",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupWorkout".to_string(),
                        arguments_json: r#"{\"workoutId\":\"workout-1\"}"#.to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-1", "Workout lookup result"),
            ],
        );

        assert_eq!(rebuilt.len(), 3);
        assert_eq!(rebuilt[1].role, LlmMessageRole::Assistant);
        assert_eq!(rebuilt[1].tool_calls.len(), 1);
        assert_eq!(rebuilt[2].role, LlmMessageRole::Tool);
    }

    #[test]
    fn next_provider_transcript_updated_at_epoch_seconds_advances_when_clock_stalls() {
        assert_eq!(
            next_provider_transcript_updated_at_epoch_seconds(42, 42),
            43
        );
    }

    #[test]
    fn timestamped_message_content_prefixes_rfc3339_timestamp() {
        let content = timestamped_message_content("Need feedback", 1_746_489_600);

        assert_eq!(
            content,
            "[sent_at=2025-05-06T00:00:00+00:00]\nNeed feedback"
        );
    }
}
