use reqwest::StatusCode;

use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::{dto::OpenRouterChatResponse, mapping};

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

#[derive(Clone)]
pub struct OpenRouterClient {
    client: reqwest::Client,
    base_url: String,
}

impl OpenRouterClient {
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

impl LlmChatPort for OpenRouterClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let payload = mapping::map_request(&config, request);

        Box::pin(async move {
            let response = client
                .post(url)
                .bearer_auth(&config.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| LlmError::Transport(error.without_url().to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(provider = "openrouter", model = %config.model, status = status.as_u16(), "openrouter chat request failed");
                return Err(map_error(status, body));
            }

            let response: OpenRouterChatResponse = response
                .json()
                .await
                .map_err(|error| LlmError::InvalidResponse(error.to_string()))?;

            mapping::map_response(&config, response)
        })
    }
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
