use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OpenRouterChatRequest {
    pub model: String,
    pub messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

#[derive(Serialize)]
pub struct OpenRouterMessage {
    pub role: String,
    pub content: String,
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
}

#[derive(Deserialize)]
pub struct OpenRouterMessageResponse {
    pub content: OpenRouterMessageContent,
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
