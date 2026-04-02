use reqwest::StatusCode;

use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::{dto::OpenAiChatResponse, mapping};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const MAX_LOGGED_RESPONSE_BODY_CHARS: usize = 400;

#[derive(Clone)]
pub struct OpenAiClient {
    client: reqwest::Client,
    base_url: String,
}

impl OpenAiClient {
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

impl LlmChatPort for OpenAiClient {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let client = self.client.clone();
        let url = format!("{}/chat/completions", self.base_url);
        let message_count = request.conversation.len();
        let has_system_prompt = !request.system_prompt.trim().is_empty();
        let has_stable_context = !request.stable_context.trim().is_empty();
        let payload = mapping::map_request(&config, request);

        Box::pin(async move {
            tracing::info!(
                provider = "openai",
                model = %config.model,
                url = %url,
                message_count,
                has_system_prompt,
                has_stable_context,
                "sending openai chat request"
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
                        provider = "openai",
                        model = %config.model,
                        url = %url,
                        error = %message,
                        "openai transport failure"
                    );
                    LlmError::Transport(message)
                })?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    provider = "openai",
                    model = %config.model,
                    url = %url,
                    status = status.as_u16(),
                    response_body = %truncate_logged_response_body(&body),
                    "openai chat request failed"
                );
                return Err(map_error(status, body));
            }

            let response_body = response.text().await.map_err(|error| {
                let message = error.without_url().to_string();
                tracing::warn!(
                    provider = "openai",
                    model = %config.model,
                    url = %url,
                    error = %message,
                    "openai response body read failed"
                );
                LlmError::InvalidResponse(message)
            })?;

            let response: OpenAiChatResponse =
                serde_json::from_str(&response_body).map_err(|error| {
                    let message = error.to_string();
                    tracing::warn!(
                        provider = "openai",
                        model = %config.model,
                        url = %url,
                        error = %message,
                        response_body = %truncate_logged_response_body(&response_body),
                        "openai response json parsing failed"
                    );
                    LlmError::InvalidResponse(message)
                })?;

            mapping::map_response(&config, response).map_err(|error| {
                tracing::warn!(
                    provider = "openai",
                    model = %config.model,
                    url = %url,
                    error = %error,
                    "openai response mapping failed"
                );
                error
            })
        })
    }
}

fn truncate_logged_response_body(body: &str) -> String {
    if body.chars().count() <= MAX_LOGGED_RESPONSE_BODY_CHARS {
        return body.to_string();
    }

    let truncated: String = body.chars().take(MAX_LOGGED_RESPONSE_BODY_CHARS).collect();
    format!("{truncated}...(truncated)")
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
