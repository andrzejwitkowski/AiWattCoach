use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: String,
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
}

#[derive(Deserialize)]
pub struct OpenAiMessageResponse {
    pub content: String,
}

#[derive(Deserialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub prompt_tokens_details: Option<OpenAiPromptTokenDetails>,
}

#[derive(Deserialize)]
pub struct OpenAiPromptTokenDetails {
    pub cached_tokens: Option<u32>,
}
