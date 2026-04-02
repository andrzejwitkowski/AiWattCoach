use std::time::Duration;

use crate::domain::llm::{
    BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProvider,
    LlmProviderConfig,
};

use super::{
    dev_adapter::DevLlmCoachAdapter, gemini::client::GeminiClient, openai::client::OpenAiClient,
    openrouter::client::OpenRouterClient,
};

#[derive(Clone)]
pub enum LlmAdapter {
    Dev(DevLlmCoachAdapter),
    Live {
        openai: OpenAiClient,
        gemini: GeminiClient,
        openrouter: OpenRouterClient,
    },
}

impl LlmAdapter {
    pub fn live(openai: OpenAiClient, gemini: GeminiClient, openrouter: OpenRouterClient) -> Self {
        Self::Live {
            openai,
            gemini,
            openrouter,
        }
    }

    fn timeout_for_model(model: &str) -> Duration {
        if is_thinking_model(model) {
            Duration::from_secs(180)
        } else {
            Duration::from_secs(60)
        }
    }
}

impl LlmChatPort for LlmAdapter {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let timeout = Self::timeout_for_model(&config.model);
        let model = config.model.clone();
        let future = match self {
            Self::Dev(adapter) => adapter.chat(config, request),
            Self::Live {
                openai,
                gemini,
                openrouter,
            } => match config.provider {
                LlmProvider::OpenAi => openai.chat(config, request),
                LlmProvider::Gemini => gemini.chat(config, request),
                LlmProvider::OpenRouter => openrouter.chat(config, request),
            },
        };

        Box::pin(async move {
            tokio::time::timeout(timeout, future).await.map_err(|_| {
                LlmError::Transport(format!(
                    "LLM request timed out after {} for model {model}",
                    format_timeout(timeout)
                ))
            })?
        })
    }
}

fn is_thinking_model(model: &str) -> bool {
    let normalized = model.trim().to_ascii_lowercase();
    normalized.starts_with("o1")
        || normalized.starts_with("o3")
        || normalized.contains("thinking")
        || normalized.contains("reason")
}

fn format_timeout(timeout: Duration) -> String {
    if timeout.as_secs() > 0 && timeout.subsec_nanos() == 0 {
        format!("{} seconds", timeout.as_secs())
    } else {
        format!("{} ms", timeout.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::llm::{LlmProvider, LlmProviderConfig};

    use super::*;

    #[test]
    fn standard_models_keep_sixty_second_timeout() {
        assert_eq!(
            LlmAdapter::timeout_for_model("gpt-4o-mini"),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn thinking_models_get_three_minute_timeout() {
        assert_eq!(
            LlmAdapter::timeout_for_model("o1-mini"),
            Duration::from_secs(180)
        );
        assert_eq!(
            LlmAdapter::timeout_for_model("gemini-2.5-pro-thinking"),
            Duration::from_secs(180)
        );
    }

    #[tokio::test]
    async fn chat_times_out_when_model_exceeds_deadline() {
        let chat = with_timeout(
            LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "o1-mini".to_string(),
                api_key: "test-key".to_string(),
            },
            Duration::from_millis(20),
            Box::pin(async {
                tokio::time::sleep(Duration::from_millis(40)).await;
                Ok(LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "o1-mini".to_string(),
                    message: "late".to_string(),
                    provider_request_id: None,
                    usage: Default::default(),
                    cache: Default::default(),
                })
            }),
        );

        let result = chat.await;
        assert_eq!(
            result,
            Err(LlmError::Transport(
                "LLM request timed out after 20 ms for model o1-mini".to_string(),
            ))
        );
    }

    #[test]
    fn thinking_model_detection_matches_supported_names() {
        assert!(is_thinking_model("o1-mini"));
        assert!(is_thinking_model("o3"));
        assert!(is_thinking_model("gemini-2.5-pro-thinking"));
        assert!(!is_thinking_model("gpt-4o-mini"));
    }

    fn with_timeout(
        config: LlmProviderConfig,
        timeout: Duration,
        future: BoxFuture<Result<LlmChatResponse, LlmError>>,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let model = config.model;
        Box::pin(async move {
            tokio::time::timeout(timeout, future).await.map_err(|_| {
                LlmError::Transport(format!(
                    "LLM request timed out after {} for model {model}",
                    format_timeout(timeout)
                ))
            })?
        })
    }
}
