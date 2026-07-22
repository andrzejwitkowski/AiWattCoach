use reqwest::StatusCode;

use crate::domain::llm::{
    normalize_openai_compatible_base_url, serialize_logged_body, truncate_logged_body, BoxFuture,
    LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProvider, LlmProviderConfig,
};

use super::{dto::OpenAiChatResponse, mapping};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    client: reqwest::Client,
    /// Dedicated client for user-supplied base URLs (no redirects).
    user_url_client: reqwest::Client,
    base_url: String,
}

impl OpenAiCompatibleClient {
    pub fn new(client: reqwest::Client) -> Self {
        let user_url_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| client.clone());
        Self {
            client,
            user_url_client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }
}

impl LlmChatPort for OpenAiCompatibleClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let uses_user_base_url = config.provider == LlmProvider::OpenAiCompatible;
        let client = if uses_user_base_url {
            self.user_url_client.clone()
        } else {
            self.client.clone()
        };
        let base_url = if uses_user_base_url {
            match config.base_url.as_deref() {
                Some(raw) => match normalize_openai_compatible_base_url(raw) {
                    Ok(url) => url,
                    Err(message) => {
                        return Box::pin(async move { Err(LlmError::ProviderRejected(message)) });
                    }
                },
                None => {
                    return Box::pin(async move {
                        Err(LlmError::ProviderRejected(
                            "OpenAI Compatible base URL is not configured".to_string(),
                        ))
                    });
                }
            }
        } else {
            self.base_url.clone()
        };
        let url = format!("{base_url}/chat/completions");
        let message_count = request.conversation.len();
        let has_system_prompt = !request.system_prompt.trim().is_empty();
        let has_stable_context = !request.stable_context.trim().is_empty();
        let provider_str = config.provider.as_str();
        let payload = match mapping::map_request(&config, request) {
            Ok(payload) => payload,
            Err(error) => return Box::pin(async move { Err(error) }),
        };

        Box::pin(async move {
            tracing::info!(
                provider = provider_str,
                model = %config.model,
                url = %url,
                message_count,
                has_system_prompt,
                has_stable_context,
                request_body = %serialize_logged_body(&payload),
                "sending openai-compatible chat request"
            );

            let response = client
                .post(url.clone())
                .bearer_auth(&config.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    let message = error.without_url().to_string();
                    tracing::warn!(
                        provider = provider_str,
                        model = %config.model,
                        url = %url,
                        error = %message,
                        "openai-compatible transport failure"
                    );
                    LlmError::Transport(message)
                })?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    provider = provider_str,
                    model = %config.model,
                    url = %url,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "openai-compatible chat request failed"
                );
                return Err(map_error(status, body));
            }

            let response_body = response.text().await.map_err(|error| {
                let message = error.without_url().to_string();
                tracing::warn!(
                    provider = provider_str,
                    model = %config.model,
                    url = %url,
                    error = %message,
                    "openai-compatible response body read failed"
                );
                LlmError::InvalidResponse(message)
            })?;

            let response: OpenAiChatResponse =
                serde_json::from_str(&response_body).map_err(|error| {
                    let message = error.to_string();
                    tracing::warn!(
                        provider = provider_str,
                        model = %config.model,
                        url = %url,
                        error = %message,
                        response_body = %truncate_logged_body(&response_body),
                        "openai-compatible response json parsing failed"
                    );
                    LlmError::InvalidResponse(message)
                })?;

            tracing::info!(
                provider = provider_str,
                model = %config.model,
                url = %url,
                response_body = %truncate_logged_body(&response_body),
                "openai-compatible chat request succeeded"
            );

            mapping::map_response(&config, response).map_err(|error| {
                tracing::warn!(
                    provider = provider_str,
                    model = %config.model,
                    url = %url,
                    error = %error,
                    response_body = %truncate_logged_body(&response_body),
                    "openai-compatible response mapping failed"
                );
                error
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::LlmChatMessage;

    #[tokio::test]
    async fn openai_compatible_requires_config_base_url() {
        let client = OpenAiCompatibleClient::new(reqwest::Client::new());
        let error = client
            .chat(
                LlmProviderConfig {
                    provider: LlmProvider::OpenAiCompatible,
                    model: "local".to_string(),
                    api_key: "key".to_string(),
                    base_url: None,
                },
                LlmChatRequest {
                    conversation: vec![LlmChatMessage::user("hi")],
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, LlmError::ProviderRejected(_)));
    }
}
