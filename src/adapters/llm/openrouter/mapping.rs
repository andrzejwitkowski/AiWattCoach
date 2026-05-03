use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmError, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
    LlmToolDefinition,
};

use crate::adapters::llm::context_prelude::non_empty_context_parts;

use super::dto::{
    OpenRouterCacheControl, OpenRouterChatRequest, OpenRouterChatResponse,
    OpenRouterFunctionDefinition, OpenRouterMessage, OpenRouterMessageContent,
    OpenRouterNamedFunctionChoice, OpenRouterNamedToolChoice, OpenRouterRequestContent,
    OpenRouterRequestContentPart, OpenRouterStringOrNumber, OpenRouterTool, OpenRouterToolCall,
    OpenRouterToolChoice as OpenRouterToolChoiceDto, OpenRouterToolFunctionCall, OpenRouterUsage,
};

pub fn map_request(config: &LlmProviderConfig, request: LlmChatRequest) -> OpenRouterChatRequest {
    let mut request = request;
    let mut messages = non_empty_context_parts([
        ("system", request.system_prompt.as_str()),
        ("system", request.stable_context.as_str()),
        ("system", request.volatile_context.as_str()),
    ])
    .into_iter()
    .map(|(role, content)| OpenRouterMessage {
        role: role.to_string(),
        content: Some(OpenRouterRequestContent::Parts(vec![
            OpenRouterRequestContentPart {
                part_type: "text".to_string(),
                text: content.to_string(),
                cache_control: Some(OpenRouterCacheControl {
                    cache_type: "ephemeral".to_string(),
                }),
            },
        ])),
        tool_calls: Vec::new(),
        tool_call_id: None,
    })
    .collect::<Vec<_>>();
    messages.extend(request.conversation.drain(..).map(map_message));

    OpenRouterChatRequest {
        model: config.model.clone(),
        messages,
        tools: request.tools.drain(..).map(map_tool_definition).collect(),
        tool_choice: map_tool_choice(request.tool_choice),
        route: None,
    }
}

pub fn map_response(
    config: &LlmProviderConfig,
    response: OpenRouterChatResponse,
) -> Result<LlmChatResponse, LlmError> {
    let choice =
        response.choices.into_iter().next().ok_or_else(|| {
            LlmError::InvalidResponse("OpenRouter returned no choices".to_string())
        })?;
    let message = map_response_message(choice.message);
    if message.content.trim().is_empty() && message.tool_calls.is_empty() {
        return Err(LlmError::InvalidResponse(
            "OpenRouter returned neither message content nor tool calls".to_string(),
        ));
    }

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
        finish_reason: choice.finish_reason.map(map_finish_reason),
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

fn map_tool_definition(tool: LlmToolDefinition) -> OpenRouterTool {
    let parameters = serde_json::from_str(&tool.input_schema_json).unwrap_or_else(|error| {
        tracing::warn!(tool_name = %tool.name, error = %error, "failed to parse tool input schema json; using permissive fallback");
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true,
        })
    });

    OpenRouterTool {
        tool_type: "function".to_string(),
        function: OpenRouterFunctionDefinition {
            name: tool.name,
            description: tool.description,
            parameters,
        },
    }
}

fn map_tool_choice(choice: LlmToolChoice) -> Option<OpenRouterToolChoiceDto> {
    match choice {
        LlmToolChoice::None => Some(OpenRouterToolChoiceDto::String("none".to_string())),
        LlmToolChoice::Auto => Some(OpenRouterToolChoiceDto::String("auto".to_string())),
        LlmToolChoice::Required => Some(OpenRouterToolChoiceDto::String("required".to_string())),
        LlmToolChoice::Named(name) => {
            Some(OpenRouterToolChoiceDto::Named(OpenRouterNamedToolChoice {
                choice_type: "function".to_string(),
                function: OpenRouterNamedFunctionChoice { name },
            }))
        }
    }
}

fn map_message(message: LlmChatMessage) -> OpenRouterMessage {
    OpenRouterMessage {
        role: match message.role {
            LlmMessageRole::System => "system".to_string(),
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "assistant".to_string(),
            LlmMessageRole::Tool => "tool".to_string(),
        },
        content: (!message.content.is_empty())
            .then_some(OpenRouterRequestContent::Text(message.content)),
        tool_calls: message
            .tool_calls
            .into_iter()
            .map(map_domain_tool_call)
            .collect(),
        tool_call_id: message.tool_call_id,
    }
}

fn map_response_message(message: super::dto::OpenRouterMessageResponse) -> LlmChatMessage {
    let content = message
        .content
        .and_then(extract_message_text)
        .unwrap_or_default();
    let tool_calls = message.tool_calls.into_iter().map(map_tool_call).collect();

    LlmChatMessage::assistant_with_tool_calls(content, tool_calls)
}

fn extract_message_text(content: OpenRouterMessageContent) -> Option<String> {
    match content {
        OpenRouterMessageContent::Text(text) => (!text.trim().is_empty()).then_some(text),
        OpenRouterMessageContent::Parts(parts) => {
            let text = parts
                .into_iter()
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join(" ");

            (!text.trim().is_empty()).then_some(text)
        }
    }
}

fn map_domain_tool_call(call: LlmToolCall) -> OpenRouterToolCall {
    OpenRouterToolCall {
        id: call.id,
        tool_type: "function".to_string(),
        function: OpenRouterToolFunctionCall {
            name: call.name,
            arguments: call.arguments_json,
        },
    }
}

fn map_tool_call(call: OpenRouterToolCall) -> LlmToolCall {
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

fn normalize_string_or_number(value: OpenRouterStringOrNumber) -> String {
    match value {
        OpenRouterStringOrNumber::String(value) => value,
        OpenRouterStringOrNumber::Number(value) => value.to_string(),
    }
}
