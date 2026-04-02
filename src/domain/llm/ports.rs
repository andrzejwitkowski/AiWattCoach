use std::{future::Future, pin::Pin};

use super::{LlmChatRequest, LlmChatResponse, LlmContextCache, LlmError, LlmProviderConfig};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait UserLlmConfigProvider: Send + Sync + 'static {
    fn get_config(&self, user_id: &str) -> BoxFuture<Result<LlmProviderConfig, LlmError>>;
}

pub trait LlmChatPort: Send + Sync + 'static {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>>;
}

pub trait LlmContextCacheRepository: Send + Sync + 'static {
    fn find_reusable(
        &self,
        user_id: &str,
        provider: &super::LlmProvider,
        model: &str,
        scope_key: &str,
        context_hash: &str,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<LlmContextCache>, LlmError>>;

    fn upsert(&self, cache: LlmContextCache) -> BoxFuture<Result<LlmContextCache, LlmError>>;
}
