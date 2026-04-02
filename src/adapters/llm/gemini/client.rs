use reqwest::StatusCode;

use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::{
    dto::{GeminiCachedContentResponse, GeminiGenerateContentResponse},
    mapping,
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const MAX_LOGGED_RESPONSE_BODY_CHARS: usize = 400;

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
        let model = config.model.clone();
        let api_model = normalize_gemini_model_name(&config.model).to_string();
        let message_count = request.conversation.len();
        let has_system_prompt = !request.system_prompt.trim().is_empty();
        let has_stable_context = !request.stable_context.trim().is_empty();

        Box::pin(async move {
            let mut provider_cache_id = request.reusable_cache_id.clone();
            let mut cache_expires_at_epoch_seconds = None;

            let can_create_cache = request.reusable_cache_id.is_none()
                && request.cache_scope_key.is_some()
                && request.cache_key.is_some();

            if can_create_cache {
                if let Some(cache_request) = mapping::map_create_cache_request(&config, &request) {
                    let cache_url = format!("{}/cachedContents?key={}", base_url, config.api_key);
                    tracing::info!(
                        provider = "gemini",
                        model = %model,
                        url = %cache_url,
                        message_count,
                        has_system_prompt,
                        has_stable_context,
                        "sending gemini cache create request"
                    );
                    let cache_response = client
                        .post(cache_url.clone())
                        .json(&cache_request)
                        .send()
                        .await
                        .map_err(|error| {
                            let message = error.without_url().to_string();
                            tracing::warn!(
                                provider = "gemini",
                                model = %model,
                                url = %cache_url,
                                error = %message,
                                "gemini cache create transport failure"
                            );
                            LlmError::Transport(message)
                        })?;

                    if cache_response.status().is_success() {
                        let cache_body = cache_response.text().await.map_err(|error| {
                            let message = error.without_url().to_string();
                            tracing::warn!(
                                provider = "gemini",
                                model = %model,
                                url = %cache_url,
                                error = %message,
                                "gemini cache create response body read failed"
                            );
                            LlmError::InvalidResponse(message)
                        })?;
                        let cache: GeminiCachedContentResponse = serde_json::from_str(&cache_body)
                            .map_err(|error| {
                                let message = error.to_string();
                                tracing::warn!(
                                    provider = "gemini",
                                    model = %model,
                                    url = %cache_url,
                                    error = %message,
                                    response_body = %truncate_logged_response_body(&cache_body),
                                    "gemini cache create json parsing failed"
                                );
                                LlmError::InvalidResponse(message)
                            })?;
                        provider_cache_id = cache.name;
                        cache_expires_at_epoch_seconds = cache
                            .expire_time
                            .as_deref()
                            .and_then(parse_expire_time_epoch_seconds);
                        tracing::info!(provider = "gemini", model = %model, cache_created = provider_cache_id.is_some(), "gemini cache create request succeeded");
                    } else {
                        let status = cache_response.status();
                        let body = cache_response.text().await.unwrap_or_default();
                        tracing::warn!(
                            provider = "gemini",
                            model = %model,
                            url = %cache_url,
                            status = status.as_u16(),
                            response_body = %truncate_logged_response_body(&body),
                            "gemini cache create request failed"
                        );
                    }
                }
            }

            let payload = mapping::map_generate_request(&request, provider_cache_id.clone());
            let generate_url = format!(
                "{}/models/{}:generateContent?key={}",
                base_url, api_model, config.api_key
            );
            tracing::info!(
                provider = "gemini",
                model = %model,
                url = %generate_url,
                message_count,
                has_system_prompt,
                has_stable_context,
                "sending gemini generate request"
            );
            let response = client
                .post(generate_url.clone())
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    let message = error.without_url().to_string();
                    tracing::warn!(
                        provider = "gemini",
                        model = %model,
                        url = %generate_url,
                        error = %message,
                        "gemini generate transport failure"
                    );
                    LlmError::Transport(message)
                })?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    provider = "gemini",
                    model = %model,
                    url = %generate_url,
                    status = status.as_u16(),
                    response_body = %truncate_logged_response_body(&body),
                    "gemini generate request failed"
                );
                return Err(map_error(status, body));
            }

            let response_body = response.text().await.map_err(|error| {
                let message = error.without_url().to_string();
                tracing::warn!(
                    provider = "gemini",
                    model = %model,
                    url = %generate_url,
                    error = %message,
                    "gemini generate response body read failed"
                );
                LlmError::InvalidResponse(message)
            })?;

            let response: GeminiGenerateContentResponse = serde_json::from_str(&response_body)
                .map_err(|error| {
                    let message = error.to_string();
                    tracing::warn!(
                        provider = "gemini",
                        model = %model,
                        url = %generate_url,
                        error = %message,
                        response_body = %truncate_logged_response_body(&response_body),
                        "gemini generate json parsing failed"
                    );
                    LlmError::InvalidResponse(message)
                })?;

            mapping::map_response(
                &config,
                response,
                provider_cache_id,
                cache_expires_at_epoch_seconds,
            )
            .map_err(|error| {
                tracing::warn!(
                    provider = "gemini",
                    model = %model,
                    url = %generate_url,
                    error = %error,
                    "gemini response mapping failed"
                );
                error
            })
        })
    }
}

fn normalize_gemini_model_name(model: &str) -> &str {
    model.strip_prefix("google/").unwrap_or(model)
}

fn truncate_logged_response_body(body: &str) -> String {
    if body.chars().count() <= MAX_LOGGED_RESPONSE_BODY_CHARS {
        return body.to_string();
    }

    let truncated: String = body.chars().take(MAX_LOGGED_RESPONSE_BODY_CHARS).collect();
    format!("{truncated}...(truncated)")
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
