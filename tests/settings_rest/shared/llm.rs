use std::future::Future;
use std::pin::Pin;

use aiwattcoach::domain::llm::{
    LlmCacheUsage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmProvider,
    LlmProviderConfig, LlmTokenUsage, UserLlmConfigProvider,
};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Clone)]
pub(crate) struct MockLlmChatService {
    result: Result<LlmChatResponse, LlmError>,
}

impl MockLlmChatService {
    pub(crate) fn returning_ok() -> Self {
        Self {
            result: Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                message: "OK".to_string(),
                provider_request_id: Some("req-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            }),
        }
    }

    pub(crate) fn returning_err(error: LlmError) -> Self {
        Self { result: Err(error) }
    }
}

impl LlmChatPort for MockLlmChatService {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        _request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestLlmConfigProvider;

impl UserLlmConfigProvider for TestLlmConfigProvider {
    fn get_config(&self, _user_id: &str) -> BoxFuture<Result<LlmProviderConfig, LlmError>> {
        Box::pin(async {
            Ok(LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                api_key: "test".to_string(),
            })
        })
    }
}
