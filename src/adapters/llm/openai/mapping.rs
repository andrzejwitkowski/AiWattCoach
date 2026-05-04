use crate::adapters::llm::context_prelude::non_empty_context_parts;
use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
    LlmToolDefinition,
};

use super::dto::{
    OpenAiChatRequest, OpenAiChatResponse, OpenAiFunctionDefinition, OpenAiMessage,
    OpenAiNamedFunctionChoice, OpenAiNamedToolChoice, OpenAiPromptTokenDetails, OpenAiTool,
    OpenAiToolCall, OpenAiToolChoice as OpenAiToolChoiceDto, OpenAiToolFunctionCall, OpenAiUsage,
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
        content: Some(content.to_string()),
        tool_calls: Vec::new(),
        tool_call_id: None,
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
        crate::domain::llm::LlmError::InvalidResponse("OpenAI returned no choices".to_string())
    })?;
    let content = choice.message.content.unwrap_or_default();
    let tool_calls = choice
        .message
        .tool_calls
        .into_iter()
        .map(map_tool_call)
        .collect::<Vec<_>>();

    if content.trim().is_empty() && tool_calls.is_empty() {
        return Err(crate::domain::llm::LlmError::InvalidResponse(
            "OpenAI returned neither message content nor tool calls".to_string(),
        ));
    }

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
        message: LlmChatMessage::assistant_with_tool_calls(content, tool_calls),
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

fn map_message(message: LlmChatMessage) -> OpenAiMessage {
    OpenAiMessage {
        role: match message.role {
            LlmMessageRole::System => "system".to_string(),
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "assistant".to_string(),
            LlmMessageRole::Tool => "tool".to_string(),
        },
        content: (!message.content.is_empty()).then_some(message.content),
        tool_calls: message
            .tool_calls
            .into_iter()
            .map(map_domain_tool_call)
            .collect(),
        tool_call_id: message.tool_call_id,
    }
}

fn map_tool_definition(
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

fn map_tool_choice(choice: LlmToolChoice) -> Option<OpenAiToolChoiceDto> {
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
