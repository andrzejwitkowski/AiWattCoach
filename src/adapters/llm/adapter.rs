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
}

impl LlmChatPort for LlmAdapter {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        match self {
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
        }
    }
}
