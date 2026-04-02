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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Debug for LlmProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmProviderConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("api_key", &redact_value(&self.api_key))
            .finish()
    }
}

impl std::fmt::Debug for LlmChatRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmChatRequest")
            .field("user_id", &self.user_id)
            .field("system_prompt", &redact_value(&self.system_prompt))
            .field("stable_context", &redact_value(&self.stable_context))
            .field("conversation_len", &self.conversation.len())
            .field("cache_scope_key", &self.cache_scope_key)
            .field("cache_key", &self.cache_key)
            .field("reusable_cache_id", &self.reusable_cache_id)
            .finish()
    }
}

impl std::fmt::Debug for LlmContextCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmContextCache")
            .field("user_id", &self.user_id)
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("scope_key", &self.scope_key)
            .field("context_hash", &redact_value(&self.context_hash))
            .field("provider_cache_id", &redact_value(&self.provider_cache_id))
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .field("created_at_epoch_seconds", &self.created_at_epoch_seconds)
            .field("updated_at_epoch_seconds", &self.updated_at_epoch_seconds)
            .finish()
    }
}

fn redact_value(value: &str) -> String {
    if value.is_empty() {
        return "<empty>".to_string();
    }

    format!("<redacted:{} chars>", value.chars().count())
}
