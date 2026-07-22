use crate::adapters::llm::context_prelude::non_empty_context_parts;
use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
    LlmToolDefinition,
};

use super::dto::{
    OpenAiChatRequest, OpenAiChatResponse, OpenAiContentPart, OpenAiFunctionDefinition,
    OpenAiImageUrl, OpenAiMessage, OpenAiMessageContent, OpenAiNamedFunctionChoice,
    OpenAiNamedToolChoice, OpenAiPromptTokenDetails, OpenAiTool, OpenAiToolCall,
    OpenAiToolChoice as OpenAiToolChoiceDto, OpenAiToolFunctionCall, OpenAiUsage,
};

const OMITTED_IMAGE_NOTE: &str =
    "[Power chart image omitted: this provider does not support OpenAI-style image inputs.]";

pub fn map_request(
    config: &LlmProviderConfig,
    request: LlmChatRequest,
) -> Result<OpenAiChatRequest, crate::domain::llm::LlmError> {
    let mut request = request;
    let include_images = provider_supports_openai_image_parts(&config.provider);
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
    messages.extend(
        request
            .conversation
            .drain(..)
            .map(|message| map_message(message, include_images)),
    );

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

pub(crate) fn map_message(message: LlmChatMessage, include_images: bool) -> OpenAiMessage {
    let LlmChatMessage {
        role,
        content,
        tool_calls,
        tool_call_id,
        reasoning_content,
        image_base64,
    } = message;
    let content = match (include_images, image_base64) {
        (true, Some(b64)) => {
            // Qwen VL docs put image before text; OpenAI accepts either order.
            let mut parts = vec![OpenAiContentPart::ImageUrl {
                image_url: OpenAiImageUrl {
                    url: format!("data:image/png;base64,{b64}"),
                    // OpenAI accepts optional detail; Qwen VL docs omit it — keep unset for compatibility.
                    detail: None,
                },
            }];
            if !content.is_empty() {
                parts.push(OpenAiContentPart::Text { text: content });
            }
            Some(OpenAiMessageContent::Parts(parts))
        }
        (false, Some(_)) => {
            let text = if content.is_empty() {
                OMITTED_IMAGE_NOTE.to_string()
            } else {
                content
            };
            Some(OpenAiMessageContent::Text(text))
        }
        (_, None) => (!content.is_empty()).then_some(OpenAiMessageContent::Text(content)),
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

fn provider_supports_openai_image_parts(provider: &LlmProvider) -> bool {
    // OpenAI Chat Completions vision format, also used by Qwen VL via openai_compatible
    // (data:image/...;base64,... URLs). Text-only models on those endpoints reject image_url.
    matches!(
        provider,
        LlmProvider::OpenAi | LlmProvider::OpenAiCompatible
    )
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
    fn map_message_with_image_base64_emits_image_url_part_for_openai() {
        let mut message = LlmChatMessage::user("Analyze this");
        message.image_base64 = Some("abc123".to_string());

        let mapped = map_message(message, true);

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
        assert_eq!(image_url.detail, None);
    }

    #[test]
    fn map_message_strips_image_when_provider_does_not_support_parts() {
        let mut message = LlmChatMessage::user("Analyze this");
        message.image_base64 = Some("abc123".to_string());

        let mapped = map_message(message, false);

        let content = mapped.content.expect("content should be present");
        assert_eq!(content.as_text(), Some("Analyze this"));
    }

    #[test]
    fn map_request_keeps_image_parts_for_openai_compatible_provider() {
        let mut message = LlmChatMessage::user("Analyze this");
        message.image_base64 = Some("abc123".to_string());

        let request = map_request(
            &LlmProviderConfig {
                provider: LlmProvider::OpenAiCompatible,
                model: "qwen3-vl-plus".to_string(),
                api_key: "key".to_string(),
                base_url: Some("https://example.com/v1".to_string()),
            },
            LlmChatRequest {
                conversation: vec![message],
                ..Default::default()
            },
        )
        .expect("request should map");

        let content = request.messages[0]
            .content
            .as_ref()
            .expect("content should be present");
        let OpenAiMessageContent::Parts(parts) = content else {
            panic!("expected parts content for openai_compatible vision");
        };
        assert!(matches!(parts[1], OpenAiContentPart::ImageUrl { .. }));
    }

    #[test]
    fn map_request_omits_image_parts_for_deepseek_provider() {
        let mut message = LlmChatMessage::user("Analyze this");
        message.image_base64 = Some("abc123".to_string());

        let request = map_request(
            &LlmProviderConfig {
                provider: LlmProvider::DeepSeek,
                model: "deepseek-chat".to_string(),
                api_key: "key".to_string(),
                base_url: None,
            },
            LlmChatRequest {
                conversation: vec![message],
                ..Default::default()
            },
        )
        .expect("request should map");

        let content = request.messages[0]
            .content
            .as_ref()
            .expect("content should be present");
        assert_eq!(content.as_text(), Some("Analyze this"));
    }

    #[test]
    fn map_message_without_image_keeps_text_content() {
        let mapped = map_message(LlmChatMessage::user("plain text"), true);

        let content = mapped.content.expect("content should be present");
        assert_eq!(content.as_text(), Some("plain text"));
    }
}
