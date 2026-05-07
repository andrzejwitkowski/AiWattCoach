use reqwest::StatusCode;

use crate::adapters::llm::logging::{serialize_logged_body, truncate_logged_body};
use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProviderConfig,
};

use super::{dto::OpenRouterChatResponse, mapping};

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_REFERER: &str = "http://localhost";
const APP_TITLE: &str = "AiWattCoach";
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
        let message_count = request.conversation.len();
        let has_system_prompt = !request.system_prompt.trim().is_empty();
        let has_stable_context = !request.stable_context.trim().is_empty();
        let payload = match mapping::map_request(&config, request) {
            Ok(payload) => payload,
            Err(error) => return Box::pin(async move { Err(error) }),
        };

        Box::pin(async move {
            tracing::info!(
                provider = "openrouter",
                model = %config.model,
                url = %url,
                message_count,
                has_system_prompt,
                has_stable_context,
                request_body = %serialize_logged_body(&payload),
                "sending openrouter chat request"
            );

            let mut attempt = 0;
            loop {
                attempt += 1;
                let (response, response_body) =
                    send_chat_request(&client, &url, &config, &payload).await?;
                let mapped = mapping::map_response(&config, response);

                match mapped {
                    Ok(response) => return Ok(response),
                    Err(error) if attempt == 1 && mapping::is_empty_response_error(&error) => {
                        tracing::warn!(
                            provider = "openrouter",
                            model = %config.model,
                            url = %url,
                            attempt,
                            error = %error,
                            response_body = %truncate_logged_body(&response_body),
                            "openrouter returned empty assistant turn, retrying once"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            provider = "openrouter",
                            model = %config.model,
                            url = %url,
                            attempt,
                            error = %error,
                            response_body = %truncate_logged_body(&response_body),
                            "openrouter response mapping failed"
                        );
                        return Err(error);
                    }
                }
            }
        })
    }
}

async fn send_chat_request(
    client: &reqwest::Client,
    url: &str,
    config: &LlmProviderConfig,
    payload: &super::dto::OpenRouterChatRequest,
) -> Result<(OpenRouterChatResponse, String), LlmError> {
    let response = client
        .post(url)
        .bearer_auth(&config.api_key)
        .header("HTTP-Referer", DEFAULT_REFERER)
        .header("X-OpenRouter-Title", APP_TITLE)
        .json(payload)
        .send()
        .await
        .map_err(|error| {
            let message = error.without_url().to_string();
            tracing::warn!(
                provider = "openrouter",
                model = %config.model,
                url = %url,
                error = %message,
                "openrouter transport failure"
            );
            LlmError::Transport(message)
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        tracing::warn!(
            provider = "openrouter",
            model = %config.model,
            url = %url,
            status = status.as_u16(),
            response_body = %truncate_logged_body(&body),
            "openrouter chat request failed"
        );
        return Err(map_error(status, body));
    }

    let response_body = response.text().await.map_err(|error| {
        let message = error.without_url().to_string();
        tracing::warn!(
            provider = "openrouter",
            model = %config.model,
            url = %url,
            error = %message,
            "openrouter response body read failed"
        );
        LlmError::InvalidResponse(message)
    })?;

    let parsed = serde_json::from_str(&response_body).map_err(|error| {
        let message = error.to_string();
        tracing::warn!(
            provider = "openrouter",
            model = %config.model,
            url = %url,
            error = %message,
            response_body = %truncate_logged_body(&response_body),
            "openrouter response json parsing failed"
        );
        LlmError::InvalidResponse(message)
    })?;

    Ok((parsed, response_body))
}

fn map_error(status: StatusCode, body: String) -> LlmError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::CredentialsNotConfigured,
        StatusCode::PAYMENT_REQUIRED => LlmError::ProviderRejected(body),
        StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimited(body),
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            LlmError::ProviderRejected(body)
        }
        _ => LlmError::Transport(body),
    }
}

#[cfg(test)]
mod tests {
    use crate::adapters::llm::logging::truncate_logged_body;

    const MAX_LOGGED_RESPONSE_BODY_CHARS: usize = 400;

    #[test]
    fn truncates_logged_openrouter_response_bodies() {
        let body = "x".repeat(MAX_LOGGED_RESPONSE_BODY_CHARS + 25);

        let truncated = truncate_logged_body(&body);

        assert_eq!(
            truncated.len(),
            MAX_LOGGED_RESPONSE_BODY_CHARS + "...(truncated)".len()
        );
        assert!(truncated.ends_with("...(truncated)"));
    }
}
