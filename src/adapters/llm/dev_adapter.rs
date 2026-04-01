use crate::domain::llm::{
    BoxFuture, LlmCacheUsage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError,
    LlmProviderConfig, LlmTokenUsage,
};

#[derive(Clone, Default)]
pub struct DevLlmCoachAdapter;

impl LlmChatPort for DevLlmCoachAdapter {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        Box::pin(async move {
            let last_user_message = request
                .conversation
                .iter()
                .rev()
                .find(|message| matches!(message.role, crate::domain::llm::LlmMessageRole::User))
                .map(|message| message.content.as_str())
                .unwrap_or("your workout");

            Ok(LlmChatResponse {
                provider: config.provider,
                model: config.model,
                message: format!(
                    "DEV coach mock: thanks for sharing. What changed most in the session after '{last_user_message}'?"
                ),
                provider_request_id: Some("dev-llm-request".to_string()),
                usage: LlmTokenUsage {
                    input_tokens: Some(128),
                    output_tokens: Some(32),
                    total_tokens: Some(160),
                },
                cache: LlmCacheUsage {
                    cached_read_tokens: Some(96),
                    cache_write_tokens: Some(0),
                    cache_hit: true,
                    cache_discount: Some("dev-cache-hit".to_string()),
                    provider_cache_id: Some("dev-cache-id".to_string()),
                    provider_cache_key: request.cache_key,
                    cache_expires_at_epoch_seconds: None,
                },
            })
        })
    }
}
