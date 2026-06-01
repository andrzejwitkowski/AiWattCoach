use serde::Serialize;

use super::{LlmChatMessage, LlmChatRequest, LlmMessageRole};

#[derive(Clone, Debug, Serialize)]
pub struct PreviewProviderMessage {
    pub role: String,
    pub content: String,
}

pub fn preview_provider_messages(request: &LlmChatRequest) -> Vec<PreviewProviderMessage> {
    let mut messages = Vec::new();
    for (role, content) in [
        ("system", request.system_prompt.as_str()),
        ("system", request.stable_context.as_str()),
        ("system", request.volatile_context.as_str()),
    ] {
        if !content.trim().is_empty() {
            messages.push(PreviewProviderMessage {
                role: role.to_string(),
                content: content.to_string(),
            });
        }
    }
    messages.extend(request.conversation.iter().map(map_conversation_message));
    messages
}

fn map_conversation_message(message: &LlmChatMessage) -> PreviewProviderMessage {
    let role = match message.role {
        LlmMessageRole::System => "system",
        LlmMessageRole::User => "user",
        LlmMessageRole::Assistant => "assistant",
        LlmMessageRole::Tool => "tool",
    };
    PreviewProviderMessage {
        role: role.to_string(),
        content: message.content.clone(),
    }
}
