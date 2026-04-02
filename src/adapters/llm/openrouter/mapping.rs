use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmError, LlmMessageRole,
    LlmProvider, LlmProviderConfig, LlmTokenUsage,
};

use super::dto::{
    OpenRouterChatRequest, OpenRouterChatResponse, OpenRouterMessage, OpenRouterMessageContent,
    OpenRouterStringOrNumber, OpenRouterUsage,
};

pub fn map_request(config: &LlmProviderConfig, request: LlmChatRequest) -> OpenRouterChatRequest {
    let mut messages = Vec::new();
    if !request.system_prompt.trim().is_empty() {
        messages.push(OpenRouterMessage {
            role: "system".to_string(),
            content: request.system_prompt,
        });
    }
    if !request.stable_context.trim().is_empty() {
        messages.push(OpenRouterMessage {
            role: "system".to_string(),
            content: request.stable_context,
        });
    }
    messages.extend(request.conversation.into_iter().map(map_message));

    OpenRouterChatRequest {
        model: config.model.clone(),
        messages,
        route: None,
    }
}

pub fn map_response(
    config: &LlmProviderConfig,
    response: OpenRouterChatResponse,
) -> Result<LlmChatResponse, LlmError> {
    let message = response
        .choices
        .into_iter()
        .next()
        .and_then(|choice| extract_message_text(choice.message.content))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            LlmError::InvalidResponse("OpenRouter returned no message content".to_string())
        })?;

    let usage = response.usage.unwrap_or(OpenRouterUsage {
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        cost: None,
        cache_discount: None,
        prompt_tokens_details: None,
    });
    let cached_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|details| details.cached_tokens);
    let cache_write_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|details| details.cache_write_tokens);

    Ok(LlmChatResponse {
        provider: LlmProvider::OpenRouter,
        model: response.model.unwrap_or_else(|| config.model.clone()),
        message,
        provider_request_id: response.id,
        usage: LlmTokenUsage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        },
        cache: LlmCacheUsage {
            cached_read_tokens: cached_tokens,
            cache_write_tokens,
            cache_hit: cached_tokens.unwrap_or(0) > 0,
            cache_discount: usage.cache_discount.map(normalize_string_or_number),
            provider_cache_id: None,
            provider_cache_key: None,
            cache_expires_at_epoch_seconds: None,
        },
    })
}

fn map_message(message: LlmChatMessage) -> OpenRouterMessage {
    OpenRouterMessage {
        role: match message.role {
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "assistant".to_string(),
        },
        content: message.content,
    }
}

fn extract_message_text(content: OpenRouterMessageContent) -> Option<String> {
    match content {
        OpenRouterMessageContent::Text(text) => Some(text),
        OpenRouterMessageContent::Parts(parts) => {
            let text = parts
                .into_iter()
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join("");

            (!text.trim().is_empty()).then_some(text)
        }
    }
}

fn normalize_string_or_number(value: OpenRouterStringOrNumber) -> String {
    match value {
        OpenRouterStringOrNumber::String(value) => value,
        OpenRouterStringOrNumber::Number(value) => value.to_string(),
    }
}
