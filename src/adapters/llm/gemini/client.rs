use reqwest::StatusCode;

use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::{
    dto::{GeminiCachedContentResponse, GeminiGenerateContentResponse},
    mapping,
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Clone)]
pub struct GeminiClient {
    client: reqwest::Client,
    base_url: String,
}

impl GeminiClient {
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

impl LlmChatPort for GeminiClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();

        Box::pin(async move {
            let mut provider_cache_id = request.reusable_cache_id.clone();
            let mut cache_expires_at_epoch_seconds = None;

            let can_create_cache = request.reusable_cache_id.is_none()
                && request.cache_scope_key.is_some()
                && request.cache_key.is_some();

            if can_create_cache {
                if let Some(cache_request) = mapping::map_create_cache_request(&config, &request) {
                    let cache_response = client
                        .post(format!(
                            "{}/cachedContents?key={}",
                            base_url, config.api_key
                        ))
                        .json(&cache_request)
                        .send()
                        .await
                        .map_err(|error| LlmError::Transport(error.without_url().to_string()))?;

                    if cache_response.status().is_success() {
                        let cache: GeminiCachedContentResponse = cache_response
                            .json()
                            .await
                            .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;
                        provider_cache_id = cache.name;
                        cache_expires_at_epoch_seconds = cache
                            .expire_time
                            .as_deref()
                            .and_then(parse_expire_time_epoch_seconds);
                        tracing::info!(provider = "gemini", model = %config.model, cache_created = provider_cache_id.is_some(), "gemini cache create request succeeded");
                    }
                }
            }

            let payload = mapping::map_generate_request(&request, provider_cache_id.clone());
            let response = client
                .post(format!(
                    "{}/models/{}:generateContent?key={}",
                    base_url, config.model, config.api_key
                ))
                .json(&payload)
                .send()
                .await
                .map_err(|error| LlmError::Transport(error.without_url().to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(provider = "gemini", model = %config.model, status = status.as_u16(), "gemini generate request failed");
                return Err(map_error(status, body));
            }

            let response: GeminiGenerateContentResponse = response
                .json()
                .await
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;

            mapping::map_response(
                &config,
                response,
                provider_cache_id,
                cache_expires_at_epoch_seconds,
            )
        })
    }
}

fn parse_expire_time_epoch_seconds(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.timestamp())
}

fn map_error(status: StatusCode, body: String) -> LlmError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::CredentialsNotConfigured,
        StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimited(body),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            LlmError::ProviderRejected(body)
        }
        _ => LlmError::Transport(body),
    }
}
