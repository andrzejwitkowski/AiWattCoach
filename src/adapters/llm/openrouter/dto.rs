use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OpenRouterChatRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<OpenRouterTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<OpenRouterToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

#[derive(Serialize)]
pub struct OpenRouterMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<OpenRouterRequestContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<OpenRouterToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize)]
pub struct OpenRouterTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenRouterFunctionDefinition,
}

#[derive(Serialize)]
pub struct OpenRouterFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum OpenRouterToolChoice {
    String(String),
    Named(OpenRouterNamedToolChoice),
}

#[derive(Serialize)]
pub struct OpenRouterNamedToolChoice {
    #[serde(rename = "type")]
    pub choice_type: String,
    pub function: OpenRouterNamedFunctionChoice,
}

#[derive(Serialize)]
pub struct OpenRouterNamedFunctionChoice {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct OpenRouterToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenRouterToolFunctionCall,
}

#[derive(Serialize, Deserialize)]
pub struct OpenRouterToolFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum OpenRouterRequestContent {
    Text(String),
    Parts(Vec<OpenRouterRequestContentPart>),
}

#[derive(Serialize)]
pub struct OpenRouterRequestContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OpenRouterCacheControl>,
}

#[derive(Clone, Serialize)]
pub struct OpenRouterCacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

#[derive(Deserialize)]
pub struct OpenRouterChatResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<OpenRouterChoice>,
    pub usage: Option<OpenRouterUsage>,
}

#[derive(Deserialize)]
pub struct OpenRouterChoice {
    pub message: OpenRouterMessageResponse,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenRouterMessageResponse {
    #[serde(default)]
    pub content: Option<OpenRouterMessageContent>,
    #[serde(default)]
    pub tool_calls: Vec<OpenRouterToolCall>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum OpenRouterMessageContent {
    Text(String),
    Parts(Vec<OpenRouterContentPart>),
}

#[derive(Deserialize)]
pub struct OpenRouterContentPart {
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenRouterUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cost: Option<OpenRouterStringOrNumber>,
    pub cache_discount: Option<OpenRouterStringOrNumber>,
    pub prompt_tokens_details: Option<OpenRouterPromptTokenDetails>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum OpenRouterStringOrNumber {
    String(String),
    Number(f64),
}

#[derive(Deserialize)]
pub struct OpenRouterPromptTokenDetails {
    pub cached_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
}
