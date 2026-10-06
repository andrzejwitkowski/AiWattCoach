use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::domain::llm::{
    llm_full_debug_logging_enabled, serialize_logged_body, truncate_logged_body, BoxFuture,
    LlmCacheUsage, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError,
    LlmFinishReason, LlmMessageRole, LlmProviderConfig, LlmTokenUsage, LlmToolCall, LlmToolChoice,
};

use super::{protocol_for_model, Protocol};

const DEFAULT_BASE_URL: &str = "https://opencode.ai/zen/go/v1";

#[derive(Clone)]
pub struct OpenCodeGoClient {
    client: reqwest::Client,
    base_url: String,
}

impl OpenCodeGoClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }
}

impl LlmChatPort for OpenCodeGoClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let Some(protocol) = protocol_for_model(&config.model) else {
            return Box::pin(async move {
                Err(LlmError::ProviderRejected(format!(
                    "OpenCode Go does not recognize model {}",
                    config.model
                )))
            });
        };
        let session_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let url = format!("{}/{}", self.base_url, endpoint(protocol));
        let payload = match map_request(protocol, &config, &request) {
            Ok(payload) => payload,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let client = self.client.clone();
        let model = config.model.clone();
        let api_key = config.api_key.clone();

        Box::pin(async move {
            if let Some(request_body) =
                request_body_for_log(&payload, llm_full_debug_logging_enabled())
            {
                tracing::info!(
                    provider = "opencode_go",
                    model = %model,
                    url = %url,
                    session_id = %session_id,
                    request_body = %request_body,
                    "sending opencode go request"
                );
            } else {
                tracing::info!(
                    provider = "opencode_go",
                    model = %model,
                    url = %url,
                    session_id = %session_id,
                    "sending opencode go request"
                );
            }
            let mut builder = client
                .post(url.clone())
                .bearer_auth(&api_key)
                .header("x-opencode-session", &session_id)
                .header(
                    reqwest::header::USER_AGENT,
                    concat!("Wattly/", env!("CARGO_PKG_VERSION")),
                )
                .json(&payload);
            if protocol == Protocol::Messages {
                builder = builder
                    .header("anthropic-version", "2023-06-01")
                    .header("x-api-key", &api_key);
            }
            let response = builder
                .send()
                .await
                .map_err(|error| LlmError::Transport(error.without_url().to_string()))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|error| LlmError::InvalidResponse(error.without_url().to_string()))?;
            if !status.is_success() {
                tracing::warn!(
                    provider = "opencode_go",
                    model = %model,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "opencode go request failed"
                );
                return Err(map_error(status, body));
            }
            let value: Value = serde_json::from_str(&body)
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;
            map_response(protocol, &config, value)
        })
    }
}

fn request_body_for_log(payload: &Value, full_debug_logging: bool) -> Option<String> {
    full_debug_logging.then(|| serialize_logged_body(payload))
}

fn endpoint(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Responses => "responses",
        Protocol::ChatCompletions => "chat/completions",
        Protocol::Messages => "messages",
    }
}

fn map_request(
    protocol: Protocol,
    config: &LlmProviderConfig,
    request: &LlmChatRequest,
) -> Result<Value, LlmError> {
    match protocol {
        Protocol::ChatCompletions => {
            let payload = crate::adapters::llm::openai_compatible::mapping::map_request(
                config,
                request.clone(),
            )?;
            serde_json::to_value(payload)
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))
        }
        Protocol::Responses => map_responses_request(config, request),
        Protocol::Messages => map_messages_request(config, request),
    }
}

fn map_responses_request(
    config: &LlmProviderConfig,
    request: &LlmChatRequest,
) -> Result<Value, LlmError> {
    let mut input = Vec::new();
    for (role, content) in [
        ("system", request.system_prompt.as_str()),
        ("system", request.stable_context.as_str()),
        ("system", request.volatile_context.as_str()),
    ] {
        if !content.trim().is_empty() {
            input.push(json!({"type":"message","role":role,"content":content}));
        }
    }
    for message in &request.conversation {
        if let Some(raw) = &message.provider_continuation_json {
            if message.role == LlmMessageRole::Assistant {
                if let Ok(items) = serde_json::from_str::<Vec<Value>>(raw) {
                    input.extend(items);
                    continue;
                }
            }
        }
        match message.role {
            LlmMessageRole::Tool => input.push(json!({
                "type":"function_call_output", "call_id":message.tool_call_id,
                "output":message.content
            })),
            LlmMessageRole::Assistant if !message.tool_calls.is_empty() => {
                if !message.content.trim().is_empty() {
                    input.push(
                        json!({"type":"message","role":"assistant","content":message.content}),
                    );
                }
                for call in &message.tool_calls {
                    input.push(json!({"type":"function_call","call_id":call.id,"name":call.name,"arguments":call.arguments_json}));
                }
            }
            role => input.push(json!({
                "type":"message",
                "role": responses_role(role),
                "content": message.content
            })),
        }
    }
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            let parameters: Value = serde_json::from_str(&tool.input_schema_json).map_err(|error| {
                LlmError::InvalidResponse(format!("invalid tool input schema for {}: {error}", tool.name))
            })?;
            Ok(json!({"type":"function","name":tool.name,"description":tool.description,"parameters":parameters,"strict":false}))
        })
        .collect::<Result<Vec<_>, LlmError>>()?;
    Ok(
        json!({"model":config.model,"input":input,"tools":tools,"tool_choice":responses_tool_choice(&request.tool_choice),"store":false}),
    )
}

fn map_messages_request(
    config: &LlmProviderConfig,
    request: &LlmChatRequest,
) -> Result<Value, LlmError> {
    let system = [
        request.system_prompt.as_str(),
        request.stable_context.as_str(),
        request.volatile_context.as_str(),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
    let mut messages = Vec::new();
    for message in &request.conversation {
        if message.role == LlmMessageRole::Assistant {
            if let Some(raw) = &message.provider_continuation_json {
                if let Ok(content) = serde_json::from_str::<Vec<Value>>(raw) {
                    messages.push(json!({"role":"assistant","content":content}));
                    continue;
                }
            }
        }
        match message.role {
            LlmMessageRole::Tool => {
                let block = json!({
                    "type":"tool_result",
                    "tool_use_id":message.tool_call_id,
                    "content":message.content
                });
                let append_to_previous = messages.last().is_some_and(|last| {
                    last["role"] == "user"
                        && last["content"].as_array().is_some_and(|content| {
                            !content.is_empty()
                                && content.iter().all(|block| block["type"] == "tool_result")
                        })
                });
                if append_to_previous {
                    if let Some(content) = messages
                        .last_mut()
                        .and_then(|last| last["content"].as_array_mut())
                    {
                        content.push(block);
                        continue;
                    }
                }
                messages.push(json!({"role":"user","content":[block]}));
            }
            LlmMessageRole::Assistant if !message.tool_calls.is_empty() => {
                let mut content = Vec::new();
                if !message.content.trim().is_empty() {
                    content.push(json!({"type":"text","text":message.content}));
                }
                for call in &message.tool_calls {
                    let input =
                        serde_json::from_str::<Value>(&call.arguments_json).map_err(|error| {
                            LlmError::InvalidResponse(format!(
                                "OpenCode Go Messages tool arguments are invalid: {error}"
                            ))
                        })?;
                    content.push(
                        json!({"type":"tool_use","id":call.id,"name":call.name,"input":input}),
                    );
                }
                messages.push(json!({"role":"assistant","content":content}));
            }
            role => messages.push(json!({"role":messages_role(role),"content":message.content})),
        }
    }
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            let input_schema: Value = serde_json::from_str(&tool.input_schema_json)
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;
            Ok(json!({"name":tool.name,"description":tool.description,"input_schema":input_schema}))
        })
        .collect::<Result<Vec<_>, LlmError>>()?;
    Ok(
        json!({"model":config.model,"max_tokens":8192,"system":system,"messages":messages,"tools":tools,"tool_choice":messages_tool_choice(&request.tool_choice)}),
    )
}

fn responses_role(role: LlmMessageRole) -> &'static str {
    match role {
        LlmMessageRole::System => "system",
        LlmMessageRole::User => "user",
        LlmMessageRole::Assistant => "assistant",
        LlmMessageRole::Tool => "user",
    }
}
fn messages_role(role: LlmMessageRole) -> &'static str {
    match role {
        LlmMessageRole::System => "user",
        LlmMessageRole::User => "user",
        LlmMessageRole::Assistant => "assistant",
        LlmMessageRole::Tool => "user",
    }
}
fn responses_tool_choice(choice: &LlmToolChoice) -> Value {
    match choice {
        LlmToolChoice::None => json!("none"),
        LlmToolChoice::Auto => json!("auto"),
        LlmToolChoice::Required => json!("required"),
        LlmToolChoice::Named(name) => json!({"type":"function","name":name}),
    }
}
fn messages_tool_choice(choice: &LlmToolChoice) -> Value {
    match choice {
        LlmToolChoice::None => json!({"type":"none"}),
        LlmToolChoice::Auto => json!({"type":"auto"}),
        LlmToolChoice::Required => json!({"type":"any"}),
        LlmToolChoice::Named(name) => json!({"type":"tool","name":name}),
    }
}

fn map_response(
    protocol: Protocol,
    config: &LlmProviderConfig,
    value: Value,
) -> Result<LlmChatResponse, LlmError> {
    match protocol {
        Protocol::ChatCompletions => {
            let response = serde_json::from_value(value)
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;
            crate::adapters::llm::openai_compatible::mapping::map_response(config, response)
        }
        Protocol::Responses => map_responses_response(config, value),
        Protocol::Messages => map_messages_response(config, value),
    }
}

fn map_responses_response(
    config: &LlmProviderConfig,
    value: Value,
) -> Result<LlmChatResponse, LlmError> {
    if value.get("status").and_then(Value::as_str) == Some("failed") {
        return Err(LlmError::InvalidResponse(
            "OpenCode Go Responses response has failed status".to_string(),
        ));
    }
    let output = value.get("output").and_then(Value::as_array);
    let mut content = String::new();
    let mut calls = Vec::new();
    for item in output.into_iter().flatten() {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                let Some(id) = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Responses function call has no call ID".to_string(),
                    ));
                };
                let Some(name) = item
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Responses function call has no name".to_string(),
                    ));
                };
                let Some(arguments_json) = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Responses function call has no arguments".to_string(),
                    ));
                };
                calls.push(LlmToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    arguments_json: arguments_json.to_string(),
                });
            }
            Some("message") => {
                if let Some(parts) = item.get("content").and_then(Value::as_array) {
                    for part in parts {
                        if part.get("type").and_then(Value::as_str) == Some("output_text") {
                            content.push_str(
                                part.get("text").and_then(Value::as_str).unwrap_or_default(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let output_text = value
        .get("output_text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if content.is_empty() {
        content = output_text.to_string();
    }
    if content.trim().is_empty() && calls.is_empty() {
        return Err(LlmError::InvalidResponse(
            "OpenCode Go Responses response has no text or function call".to_string(),
        ));
    }
    let has_tool_calls = !calls.is_empty();
    let mut message = LlmChatMessage::assistant_with_tool_calls(content, calls);
    message.provider_continuation_json = value
        .get("output")
        .filter(|output| output.is_array())
        .and_then(|output| serde_json::to_string(output).ok());
    let usage = value.get("usage");
    Ok(LlmChatResponse {
        provider: config.provider.clone(),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&config.model)
            .to_string(),
        message,
        finish_reason: Some(
            if value.get("status").and_then(Value::as_str) == Some("incomplete") {
                LlmFinishReason::Unknown("incomplete".to_string())
            } else if !has_tool_calls {
                LlmFinishReason::Stop
            } else {
                LlmFinishReason::ToolCalls
            },
        ),
        provider_request_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        usage: token_usage(usage),
        cache: LlmCacheUsage::default(),
    })
}

fn map_messages_response(
    config: &LlmProviderConfig,
    value: Value,
) -> Result<LlmChatResponse, LlmError> {
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            LlmError::InvalidResponse("OpenCode Go Messages response has no content".to_string())
        })?;
    let mut content = String::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => content.push_str(
                block
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            Some("tool_use") => {
                let Some(id) = block
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Messages tool call has no call ID".to_string(),
                    ));
                };
                let Some(name) = block
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Messages tool call has no name".to_string(),
                    ));
                };
                let Some(input) = block.get("input") else {
                    return Err(LlmError::InvalidResponse(
                        "OpenCode Go Messages tool call has no input".to_string(),
                    ));
                };
                calls.push(LlmToolCall {
                    id: id.to_string(),
                    name: name.to_string(),
                    arguments_json: serde_json::to_string(input).map_err(|error| {
                        LlmError::InvalidResponse(format!(
                            "OpenCode Go Messages tool arguments are invalid: {error}"
                        ))
                    })?,
                });
            }
            _ => {}
        }
    }
    if content.trim().is_empty() && calls.is_empty() {
        return Err(LlmError::InvalidResponse(
            "OpenCode Go Messages response has no text or function call".to_string(),
        ));
    }
    let finish = match value.get("stop_reason").and_then(Value::as_str) {
        Some("tool_use") => LlmFinishReason::ToolCalls,
        Some("max_tokens") => LlmFinishReason::Length,
        _ if !calls.is_empty() => LlmFinishReason::ToolCalls,
        _ => LlmFinishReason::Stop,
    };
    let mut message = LlmChatMessage::assistant_with_tool_calls(content, calls);
    message.provider_continuation_json = value
        .get("content")
        .filter(|content| content.is_array())
        .and_then(|content| serde_json::to_string(content).ok());
    Ok(LlmChatResponse {
        provider: config.provider.clone(),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&config.model)
            .to_string(),
        message,
        finish_reason: Some(finish),
        provider_request_id: value.get("id").and_then(Value::as_str).map(str::to_string),
        usage: token_usage(value.get("usage")),
        cache: LlmCacheUsage::default(),
    })
}

fn token_usage(value: Option<&Value>) -> LlmTokenUsage {
    let Some(value) = value else {
        return LlmTokenUsage::default();
    };
    LlmTokenUsage {
        input_tokens: value
            .get("input_tokens")
            .or_else(|| {
                value
                    .get("input_tokens_details")
                    .and_then(|v| v.get("input_tokens"))
            })
            .and_then(Value::as_u64)
            .map(|v| v as u32)
            .or_else(|| {
                value
                    .get("prompt_tokens")
                    .and_then(Value::as_u64)
                    .map(|v| v as u32)
            }),
        output_tokens: value
            .get("output_tokens")
            .or_else(|| value.get("completion_tokens"))
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        total_tokens: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
    }
}

fn map_error(status: StatusCode, body: String) -> LlmError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            LlmError::provider_auth_rejected(status.as_u16(), &body)
        }
        StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimited(body),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            LlmError::ProviderRejected(body)
        }
        _ => LlmError::Transport(body),
    }
}

#[cfg(test)]
mod tests;
