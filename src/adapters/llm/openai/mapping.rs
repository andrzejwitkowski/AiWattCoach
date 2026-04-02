use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmMessageRole, LlmProvider,
    LlmProviderConfig, LlmTokenUsage,
};

use super::dto::{
    OpenAiChatRequest, OpenAiChatResponse, OpenAiMessage, OpenAiPromptTokenDetails, OpenAiUsage,
};

pub fn map_request(config: &LlmProviderConfig, request: LlmChatRequest) -> OpenAiChatRequest {
    let mut messages = Vec::new();
    if !request.system_prompt.trim().is_empty() {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: request.system_prompt,
        });
    }
    if !request.stable_context.trim().is_empty() {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: request.stable_context,
        });
    }
    messages.extend(request.conversation.into_iter().map(map_message));

    OpenAiChatRequest {
        model: config.model.clone(),
        messages,
        prompt_cache_key: request.cache_key,
    }
}

pub fn map_response(
    config: &LlmProviderConfig,
    response: OpenAiChatResponse,
) -> Result<LlmChatResponse, crate::domain::llm::LlmError> {
    let message = response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            crate::domain::llm::LlmError::InvalidResponse(
                "OpenAI returned no message content".to_string(),
            )
        })?;

    let usage = response.usage.unwrap_or(OpenAiUsage {
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        prompt_tokens_details: None,
    });
    let prompt_details = usage
        .prompt_tokens_details
        .unwrap_or(OpenAiPromptTokenDetails {
            cached_tokens: None,
        });
    let cached_tokens = prompt_details.cached_tokens;

    Ok(LlmChatResponse {
        provider: LlmProvider::OpenAi,
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
            cache_write_tokens: None,
            cache_hit: cached_tokens.unwrap_or(0) > 0,
            cache_discount: None,
            provider_cache_id: None,
            provider_cache_key: None,
            cache_expires_at_epoch_seconds: None,
        },
    })
}

fn map_message(message: LlmChatMessage) -> OpenAiMessage {
    OpenAiMessage {
        role: match message.role {
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "assistant".to_string(),
        },
        content: message.content,
    }
}
