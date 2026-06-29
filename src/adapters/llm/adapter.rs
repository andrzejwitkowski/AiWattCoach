use std::time::Duration;

use crate::domain::llm::{
    llm_request_timeout, BoxFuture, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError,
    LlmProvider, LlmProviderConfig,
};

use super::{
    dev_adapter::DevLlmCoachAdapter, gemini::client::GeminiClient,
    openai_compatible::client::OpenAiCompatibleClient, openrouter::client::OpenRouterClient,
    zai::client::ZaiClient,
};

#[derive(Clone)]
pub enum LlmAdapter {
    Dev(DevLlmCoachAdapter),
    Live {
        openai: OpenAiCompatibleClient,
        deepseek: OpenAiCompatibleClient,
        zai: ZaiClient,
        gemini: GeminiClient,
        openrouter: OpenRouterClient,
    },
}

impl LlmAdapter {
    pub fn live(
        openai: OpenAiCompatibleClient,
        deepseek: OpenAiCompatibleClient,
        zai: ZaiClient,
        gemini: GeminiClient,
        openrouter: OpenRouterClient,
    ) -> Self {
        Self::Live {
            openai,
            deepseek,
            zai,
            gemini,
            openrouter,
        }
    }

    fn timeout_for_model(_model: &str) -> Duration {
        llm_request_timeout()
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
                deepseek,
                zai,
                gemini,
                openrouter,
            } => match config.provider {
                LlmProvider::OpenAi => openai.chat(config, request),
                LlmProvider::DeepSeek => deepseek.chat(config, request),
                LlmProvider::Zai => zai.chat(config, request),
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
    fn standard_models_use_shared_three_minute_timeout() {
        assert_eq!(
            LlmAdapter::timeout_for_model("gpt-4o-mini"),
            Duration::from_secs(180)
        );
    }

    #[test]
    fn thinking_models_use_same_shared_three_minute_timeout() {
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
                    message: crate::domain::llm::LlmChatMessage::assistant("late"),
                    finish_reason: None,
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
