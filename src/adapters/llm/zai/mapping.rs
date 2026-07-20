use crate::adapters::llm::openai_compatible::dto::{
    OpenAiChatRequest, OpenAiMessage, OpenAiMessageContent,
};
use crate::adapters::llm::openai_compatible::mapping::{
    map_message, map_tool_choice, map_tool_definition,
};
use crate::domain::llm::{LlmChatMessage, LlmChatRequest, LlmMessageRole, LlmProviderConfig};

pub fn map_zai_request(
    config: &LlmProviderConfig,
    request: LlmChatRequest,
) -> Result<OpenAiChatRequest, crate::domain::llm::LlmError> {
    let mut request = request;
    let prompt_cache_key = request.cache_key.clone();
    let volatile_context = request.volatile_context.trim().to_string();

    let merged_system = [
        request.system_prompt.as_str(),
        request.stable_context.as_str(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    let mut messages = Vec::new();
    if !merged_system.is_empty() {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: Some(OpenAiMessageContent::Text(merged_system)),
            tool_calls: Vec::new(),
            tool_call_id: None,
            reasoning_content: None,
        });
    }

    let mut conversation = request.conversation;
    if !volatile_context.is_empty() {
        append_volatile_to_last_user(&mut conversation, &volatile_context);
    }
    messages.extend(conversation.drain(..).map(map_message));

    Ok(OpenAiChatRequest {
        model: config.model.clone(),
        messages,
        tools: request
            .tools
            .drain(..)
            .map(map_tool_definition)
            .collect::<Result<Vec<_>, _>>()?,
        tool_choice: map_tool_choice(request.tool_choice),
        prompt_cache_key,
    })
}

fn append_volatile_to_last_user(conversation: &mut Vec<LlmChatMessage>, volatile_context: &str) {
    if let Some(last_user) = conversation
        .iter_mut()
        .rev()
        .find(|message| message.role == LlmMessageRole::User)
    {
        if last_user.content.trim().is_empty() {
            last_user.content = volatile_context.to_string();
        } else {
            last_user.content = format!("{}\n\n{}", last_user.content, volatile_context);
        }
        return;
    }

    conversation.push(LlmChatMessage::user(volatile_context));
}

#[cfg(test)]
mod tests {
    use crate::domain::llm::{LlmChatMessage, LlmChatRequest, LlmProvider, LlmProviderConfig};

    use super::map_zai_request;

    fn zai_config() -> LlmProviderConfig {
        LlmProviderConfig {
            provider: LlmProvider::Zai,
            model: "glm-5.2".to_string(),
            api_key: "zai-key".to_string(),
            base_url: None,
        }
    }

    #[test]
    fn map_zai_request_merges_system_and_stable_context() {
        let payload = map_zai_request(
            &zai_config(),
            LlmChatRequest {
                system_prompt: "coach".to_string(),
                stable_context: "packed=1".to_string(),
                volatile_context: String::new(),
                conversation: vec![LlmChatMessage::user("hello")],
                cache_key: Some("cache-hash".to_string()),
                ..Default::default()
            },
        )
        .expect("request should map");

        assert_eq!(payload.messages.len(), 2);
        assert_eq!(payload.messages[0].role, "system");
        assert_eq!(
            payload.messages[0]
                .content
                .as_ref()
                .and_then(|c| c.as_text()),
            Some("coach\n\npacked=1")
        );
        assert_eq!(payload.prompt_cache_key.as_deref(), Some("cache-hash"));
    }

    #[test]
    fn map_zai_request_appends_volatile_to_last_user_message() {
        let payload = map_zai_request(
            &zai_config(),
            LlmChatRequest {
                system_prompt: "coach".to_string(),
                stable_context: "packed=1".to_string(),
                volatile_context: "timing=now".to_string(),
                conversation: vec![
                    LlmChatMessage::user("first"),
                    LlmChatMessage::assistant("reply"),
                    LlmChatMessage::user("second"),
                ],
                ..Default::default()
            },
        )
        .expect("request should map");

        assert_eq!(payload.messages.len(), 4);
        assert_eq!(payload.messages[3].role, "user");
        assert_eq!(
            payload.messages[3]
                .content
                .as_ref()
                .and_then(|c| c.as_text()),
            Some("second\n\ntiming=now")
        );
    }

    #[test]
    fn map_zai_request_creates_user_message_when_only_volatile_present() {
        let payload = map_zai_request(
            &zai_config(),
            LlmChatRequest {
                volatile_context: "timing=now".to_string(),
                ..Default::default()
            },
        )
        .expect("request should map");

        assert_eq!(payload.messages.len(), 1);
        assert_eq!(payload.messages[0].role, "user");
        assert_eq!(
            payload.messages[0]
                .content
                .as_ref()
                .and_then(|c| c.as_text()),
            Some("timing=now")
        );
    }

    #[test]
    fn map_zai_request_preserves_stable_prefix_across_turns() {
        let base = LlmChatRequest {
            system_prompt: "coach".to_string(),
            stable_context: "packed=1".to_string(),
            volatile_context: "timing=turn-1".to_string(),
            cache_key: Some("cache-hash".to_string()),
            ..Default::default()
        };

        let turn_one = map_zai_request(
            &zai_config(),
            LlmChatRequest {
                conversation: vec![LlmChatMessage::user("hello")],
                ..base.clone()
            },
        )
        .expect("turn one should map");
        let turn_two = map_zai_request(
            &zai_config(),
            LlmChatRequest {
                conversation: vec![
                    LlmChatMessage::user("hello"),
                    LlmChatMessage::assistant("hi"),
                    LlmChatMessage::user("follow up"),
                ],
                volatile_context: "timing=turn-2".to_string(),
                ..base
            },
        )
        .expect("turn two should map");

        assert_eq!(turn_one.messages[0].content, turn_two.messages[0].content);
        assert_eq!(
            turn_two.messages[1]
                .content
                .as_ref()
                .and_then(|c| c.as_text()),
            Some("hello")
        );
        assert_eq!(
            turn_two.messages[2]
                .content
                .as_ref()
                .and_then(|c| c.as_text()),
            Some("hi")
        );
        assert_ne!(
            turn_one
                .messages
                .last()
                .and_then(|m| m.content.as_ref().and_then(|c| c.as_text())),
            turn_two
                .messages
                .last()
                .and_then(|m| m.content.as_ref().and_then(|c| c.as_text()))
        );
    }
}
