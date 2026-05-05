use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LLM_REQUEST_TIMEOUT_SECONDS: u64 = 180;

pub fn llm_request_timeout() -> Duration {
    Duration::from_secs(LLM_REQUEST_TIMEOUT_SECONDS)
}

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
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LlmToolChoice {
    #[default]
    None,
    Auto,
    Required,
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmChatMessage {
    pub role: LlmMessageRole,
    pub content: String,
    pub tool_calls: Vec<LlmToolCall>,
    pub tool_call_id: Option<String>,
}

impl LlmChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlmMessageRole::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlmMessageRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlmMessageRole::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<LlmToolCall>,
    ) -> Self {
        Self {
            role: LlmMessageRole::Assistant,
            content: content.into(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: LlmMessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LlmChatRequest {
    pub user_id: String,
    pub system_prompt: String,
    pub stable_context: String,
    pub volatile_context: String,
    pub conversation: Vec<LlmChatMessage>,
    pub cache_scope_key: Option<String>,
    pub cache_key: Option<String>,
    pub reusable_cache_id: Option<String>,
    /// Populated by the orchestrator (e.g. `run_tool_loop`) based on scope.
    /// Do NOT set this in domain request builders; it is injected at the
    /// transport boundary so the builder is not misleading about whether
    /// tools are actually available.
    #[serde(default)]
    pub tools: Vec<LlmToolDefinition>,
    /// Populated by the orchestrator (e.g. `run_tool_loop`) based on scope.
    /// See note on `tools` above.
    #[serde(default)]
    pub tool_choice: LlmToolChoice,
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
pub enum LlmFinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmChatResponse {
    pub provider: LlmProvider,
    pub model: String,
    pub message: LlmChatMessage,
    pub finish_reason: Option<LlmFinishReason>,
    pub provider_request_id: Option<String>,
    pub usage: LlmTokenUsage,
    pub cache: LlmCacheUsage,
}

impl LlmChatResponse {
    pub fn assistant_text(&self) -> Option<&str> {
        match self.message.role {
            LlmMessageRole::Assistant => Some(self.message.content.as_str()),
            _ => None,
        }
    }

    pub fn tool_calls(&self) -> &[LlmToolCall] {
        self.message.tool_calls.as_slice()
    }
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

pub fn approximate_token_budget_for_model(model: &str) -> usize {
    let normalized = normalize_model_name(model);

    if normalized.starts_with("o1") || normalized.starts_with("o3") {
        return 100_000;
    }

    if normalized.contains("gemini-2.5-pro") || normalized.contains("gemini-3.1-pro") {
        return 120_000;
    }

    if normalized.contains("gemini") {
        return 32_000;
    }

    if normalized.contains("gpt-4o")
        || normalized.contains("gpt-4.5")
        || normalized.contains("gpt-5")
    {
        return 28_000;
    }

    if normalized.contains("claude") {
        return 32_000;
    }

    24_000
}

fn normalize_model_name(model: &str) -> String {
    model.trim().to_ascii_lowercase().replace([' ', '_'], "-")
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
            .field("volatile_context", &redact_value(&self.volatile_context))
            .field("conversation_len", &self.conversation.len())
            .field("tools_len", &self.tools.len())
            .field("tool_choice", &self.tool_choice)
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

#[cfg(test)]
mod tests {
    use super::approximate_token_budget_for_model;

    #[test]
    fn approximate_token_budget_recognizes_frontier_models() {
        assert_eq!(approximate_token_budget_for_model("gpt-4.5"), 28_000);
        assert_eq!(approximate_token_budget_for_model("openai/gpt-4.5"), 28_000);
        assert_eq!(
            approximate_token_budget_for_model("gemini-3.1-pro"),
            120_000
        );
        assert_eq!(
            approximate_token_budget_for_model("google/gemini 3.1 pro"),
            120_000
        );
    }
}
