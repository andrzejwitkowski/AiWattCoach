use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmProvider {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::OpenRouter => "openrouter",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAi),
            "gemini" => Some(Self::Gemini),
            "openrouter" => Some(Self::OpenRouter),
            _ => None,
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-4o-mini",
            Self::Gemini => "gemini-2.5-flash",
            Self::OpenRouter => "openai/gpt-4o-mini",
        }
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub api_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmMessageRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmChatMessage {
    pub role: LlmMessageRole,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmChatRequest {
    pub user_id: String,
    pub system_prompt: String,
    pub stable_context: String,
    pub conversation: Vec<LlmChatMessage>,
    pub cache_scope_key: Option<String>,
    pub cache_key: Option<String>,
    pub reusable_cache_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LlmTokenUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LlmCacheUsage {
    pub cached_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub cache_hit: bool,
    pub cache_discount: Option<String>,
    pub provider_cache_id: Option<String>,
    pub provider_cache_key: Option<String>,
    pub cache_expires_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmChatResponse {
    pub provider: LlmProvider,
    pub model: String,
    pub message: String,
    pub provider_request_id: Option<String>,
    pub usage: LlmTokenUsage,
    pub cache: LlmCacheUsage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmContextCache {
    pub user_id: String,
    pub provider: LlmProvider,
    pub model: String,
    pub scope_key: String,
    pub context_hash: String,
    pub provider_cache_id: String,
    pub expires_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

pub fn hash_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{digest:x}")
}
