use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<OpenAiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OpenAiToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunctionDefinition,
}

#[derive(Serialize)]
pub struct OpenAiFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum OpenAiToolChoice {
    String(String),
    Named(OpenAiNamedToolChoice),
}

#[derive(Serialize)]
pub struct OpenAiNamedToolChoice {
    #[serde(rename = "type")]
    pub choice_type: String,
    pub function: OpenAiNamedFunctionChoice,
}

#[derive(Serialize)]
pub struct OpenAiNamedFunctionChoice {
    pub name: String,
}

#[derive(Serialize)]
pub struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<OpenAiToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct OpenAiToolFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Deserialize)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    pub function: OpenAiToolFunctionCall,
}

#[derive(Deserialize)]
pub struct OpenAiChatResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<OpenAiChoice>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
pub struct OpenAiChoice {
    pub message: OpenAiMessageResponse,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenAiMessageResponse {
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Deserialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub prompt_tokens_details: Option<OpenAiPromptTokenDetails>,
    #[serde(default)]
    pub prompt_cache_hit_tokens: Option<u32>,
    #[serde(default)]
    pub prompt_cache_miss_tokens: Option<u32>,
}

#[derive(Deserialize)]
pub struct OpenAiPromptTokenDetails {
    pub cached_tokens: Option<u32>,
}
