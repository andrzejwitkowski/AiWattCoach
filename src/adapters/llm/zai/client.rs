use reqwest::StatusCode;

use crate::domain::llm::{
    serialize_logged_body, truncate_logged_body, BoxFuture, LlmChatPort, LlmChatRequest,
    LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::mapping::map_zai_request;
use crate::adapters::llm::openai_compatible::dto::OpenAiChatResponse;
use crate::adapters::llm::openai_compatible::mapping::map_response;

const DEFAULT_BASE_URL: &str = "https://api.z.ai/api/paas/v4";

#[derive(Clone)]
pub struct ZaiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ZaiClient {
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

impl LlmChatPort for ZaiClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let provider_str = config.provider.as_str();
        let cache_scope_key = request.cache_scope_key.clone();
        let is_follow_up_turn = request.conversation.len() > 1;
        let payload = match map_zai_request(&config, request) {
            Ok(payload) => payload,
            Err(error) => return Box::pin(async move { Err(error) }),
        };

        Box::pin(async move {
            tracing::info!(
                provider = provider_str,
                model = %config.model,
                url = %url,
                request_body = %serialize_logged_body(&payload),
                "sending z.ai chat request"
            );

            let response = client
                .post(url.clone())
                .bearer_auth(&config.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| LlmError::Transport(error.without_url().to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    provider = provider_str,
                    model = %config.model,
                    url = %url,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "z.ai chat request failed"
                );
                return Err(map_error(status, body));
            }

            let response_body = response
                .text()
                .await
                .map_err(|error| LlmError::InvalidResponse(error.without_url().to_string()))?;

            let response: OpenAiChatResponse =
                serde_json::from_str(&response_body).map_err(|error| {
                    LlmError::InvalidResponse(format!(
                        "{provider_str} response json parsing failed: {error}; body={}",
                        truncate_logged_body(&response_body)
                    ))
                })?;

            let mapped = map_response(&config, response)?;

            let input_tokens = mapped.usage.input_tokens.unwrap_or(0);
            tracing::info!(
                provider = provider_str,
                model = %mapped.model,
                cache_scope_key = cache_scope_key.as_deref().unwrap_or(""),
                input_tokens,
                cached_read_tokens = mapped.cache.cached_read_tokens.unwrap_or(0),
                cache_hit = mapped.cache.cache_hit,
                "z.ai chat request succeeded"
            );

            if is_follow_up_turn && input_tokens > 1000 && !mapped.cache.cache_hit {
                tracing::warn!(
                    provider = provider_str,
                    model = %mapped.model,
                    cache_scope_key = cache_scope_key.as_deref().unwrap_or(""),
                    input_tokens,
                    "z.ai follow-up request missed implicit prefix cache"
                );
            }

            Ok(mapped)
        })
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
