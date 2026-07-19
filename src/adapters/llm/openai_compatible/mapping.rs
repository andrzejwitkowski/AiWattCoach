use crate::adapters::llm::context_prelude::non_empty_context_parts;
use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmFinishReason,
    LlmMessageRole, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
    LlmToolDefinition,
};

use super::dto::{
    OpenAiChatRequest, OpenAiChatResponse, OpenAiContentPart, OpenAiFunctionDefinition,
    OpenAiImageUrl, OpenAiMessage, OpenAiMessageContent, OpenAiNamedFunctionChoice,
    OpenAiNamedToolChoice, OpenAiPromptTokenDetails, OpenAiTool, OpenAiToolCall,
    OpenAiToolChoice as OpenAiToolChoiceDto, OpenAiToolFunctionCall, OpenAiUsage,
};

pub fn map_request(
    config: &LlmProviderConfig,
    request: LlmChatRequest,
) -> Result<OpenAiChatRequest, crate::domain::llm::LlmError> {
    let mut request = request;
    let mut messages = non_empty_context_parts([
        ("system", request.system_prompt.as_str()),
        ("system", request.stable_context.as_str()),
        ("system", request.volatile_context.as_str()),
    ])
    .into_iter()
    .map(|(role, content)| OpenAiMessage {
        role: role.to_string(),
        content: Some(OpenAiMessageContent::Text(content.to_string())),
        tool_calls: Vec::new(),
        tool_call_id: None,
        reasoning_content: None,
    })
    .collect::<Vec<_>>();
    messages.extend(request.conversation.drain(..).map(map_message));

    Ok(OpenAiChatRequest {
        model: config.model.clone(),
        messages,
        tools: request
            .tools
            .drain(..)
            .map(map_tool_definition)
            .collect::<Result<Vec<_>, _>>()?,
        tool_choice: map_tool_choice(request.tool_choice),
        prompt_cache_key: request.cache_key,
    })
}

pub fn map_response(
    config: &LlmProviderConfig,
    response: OpenAiChatResponse,
) -> Result<LlmChatResponse, crate::domain::llm::LlmError> {
    let choice = response.choices.into_iter().next().ok_or_else(|| {
        crate::domain::llm::LlmError::InvalidResponse(format!(
            "{} returned no choices",
            config.provider
        ))
    })?;
    let content = choice.message.content.unwrap_or_default();
    let tool_calls = choice
        .message
        .tool_calls
        .into_iter()
        .map(map_tool_call)
        .collect::<Vec<_>>();
    let has_reasoning = choice
        .message
        .reasoning_content
        .as_ref()
        .is_some_and(|r| !r.trim().is_empty());

    if content.trim().is_empty() && tool_calls.is_empty() && !has_reasoning {
        return Err(crate::domain::llm::LlmError::InvalidResponse(format!(
            "{} returned neither message content nor tool calls",
            config.provider
        )));
    }

    let usage = response.usage.unwrap_or(OpenAiUsage {
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        prompt_tokens_details: None,
        prompt_cache_hit_tokens: None,
        prompt_cache_miss_tokens: None,
    });
    let prompt_details = usage
        .prompt_tokens_details
        .unwrap_or(OpenAiPromptTokenDetails {
            cached_tokens: None,
        });
    let cached_tokens = prompt_details
        .cached_tokens
        .or(usage.prompt_cache_hit_tokens);

    let mut message = LlmChatMessage::assistant_with_tool_calls(content, tool_calls);
    message.reasoning_content = choice.message.reasoning_content;

    Ok(LlmChatResponse {
        provider: config.provider.clone(),
        model: response.model.unwrap_or_else(|| config.model.clone()),
        message,
        finish_reason: choice.finish_reason.map(map_finish_reason),
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

pub(crate) fn map_message(message: LlmChatMessage) -> OpenAiMessage {
    let LlmChatMessage {
        role,
        content,
        tool_calls,
        tool_call_id,
        reasoning_content,
        image_base64,
    } = message;
    let content = match image_base64 {
        Some(b64) => {
            let mut parts = Vec::new();
            if !content.is_empty() {
                parts.push(OpenAiContentPart::Text { text: content });
            }
            parts.push(OpenAiContentPart::ImageUrl {
                image_url: OpenAiImageUrl {
                    url: format!("data:image/png;base64,{b64}"),
                    detail: Some("high".to_string()),
                },
            });
            Some(OpenAiMessageContent::Parts(parts))
        }
        None => (!content.is_empty()).then_some(OpenAiMessageContent::Text(content)),
    };
    OpenAiMessage {
        role: match role {
            LlmMessageRole::System => "system".to_string(),
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "assistant".to_string(),
            LlmMessageRole::Tool => "tool".to_string(),
        },
        content,
        tool_calls: tool_calls.into_iter().map(map_domain_tool_call).collect(),
        tool_call_id,
        reasoning_content,
    }
}

pub(crate) fn map_tool_definition(
    tool: LlmToolDefinition,
) -> Result<OpenAiTool, crate::domain::llm::LlmError> {
    let parameters = serde_json::from_str(&tool.input_schema_json).map_err(|error| {
        crate::domain::llm::LlmError::InvalidResponse(format!(
            "invalid tool input schema for {}: {error}",
            tool.name
        ))
    })?;

    Ok(OpenAiTool {
        tool_type: "function".to_string(),
        function: OpenAiFunctionDefinition {
            name: tool.name,
            description: tool.description,
            parameters,
        },
    })
}

pub(crate) fn map_tool_choice(choice: LlmToolChoice) -> Option<OpenAiToolChoiceDto> {
    match choice {
        LlmToolChoice::None => Some(OpenAiToolChoiceDto::String("none".to_string())),
        LlmToolChoice::Auto => Some(OpenAiToolChoiceDto::String("auto".to_string())),
        LlmToolChoice::Required => Some(OpenAiToolChoiceDto::String("required".to_string())),
        LlmToolChoice::Named(name) => Some(OpenAiToolChoiceDto::Named(OpenAiNamedToolChoice {
            choice_type: "function".to_string(),
            function: OpenAiNamedFunctionChoice { name },
        })),
    }
}

fn map_domain_tool_call(call: LlmToolCall) -> OpenAiToolCall {
    OpenAiToolCall {
        id: call.id,
        tool_type: Some("function".to_string()),
        function: OpenAiToolFunctionCall {
            name: call.name,
            arguments: call.arguments_json,
        },
    }
}

fn map_tool_call(call: OpenAiToolCall) -> LlmToolCall {
    LlmToolCall {
        id: call.id,
        name: call.function.name,
        arguments_json: call.function.arguments,
    }
}

fn map_finish_reason(value: String) -> LlmFinishReason {
    match value.as_str() {
        "stop" => LlmFinishReason::Stop,
        "length" => LlmFinishReason::Length,
        "tool_calls" => LlmFinishReason::ToolCalls,
        "content_filter" => LlmFinishReason::ContentFilter,
        other => LlmFinishReason::Unknown(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_message_with_image_base64_emits_image_url_part() {
        let mut message = LlmChatMessage::user("Analyze this");
        message.image_base64 = Some("abc123".to_string());

        let mapped = map_message(message);

        let content = mapped.content.expect("content should be present");
        let OpenAiMessageContent::Parts(parts) = content else {
            panic!("expected parts content for image message");
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], OpenAiContentPart::Text { .. }));
        let OpenAiContentPart::ImageUrl { image_url } = &parts[1] else {
            panic!("expected image_url part");
        };
        assert_eq!(image_url.url, "data:image/png;base64,abc123");
        assert_eq!(image_url.detail.as_deref(), Some("high"));
    }

    #[test]
    fn map_message_without_image_keeps_text_content() {
        let mapped = map_message(LlmChatMessage::user("plain text"));

        let content = mapped.content.expect("content should be present");
        assert_eq!(content.as_text(), Some("plain text"));
    }
}
