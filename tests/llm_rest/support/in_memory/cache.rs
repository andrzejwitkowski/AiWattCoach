use super::*;

#[derive(Clone, Default)]
pub(crate) struct InMemoryLlmContextCacheRepository {
    caches: Arc<Mutex<Vec<LlmContextCache>>>,
}

impl LlmContextCacheRepository for InMemoryLlmContextCacheRepository {
    fn find_reusable(
        &self,
        user_id: &str,
        provider: &aiwattcoach::domain::llm::LlmProvider,
        model: &str,
        scope_key: &str,
        context_hash: &str,
        now_epoch_seconds: i64,
    ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        let caches = self.caches.clone();
        let user_id = user_id.to_string();
        let provider = provider.clone();
        let model = model.to_string();
        let scope_key = scope_key.to_string();
        let context_hash = context_hash.to_string();
        Box::pin(async move {
            Ok(caches
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|cache| {
                    cache.user_id == user_id
                        && cache.provider == provider
                        && cache.model == model
                        && cache.scope_key == scope_key
                        && cache.context_hash == context_hash
                        && cache
                            .expires_at_epoch_seconds
                            .is_none_or(|expires_at| expires_at > now_epoch_seconds)
                })
                .cloned())
        })
    }

    fn upsert(&self, cache: LlmContextCache) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
        let caches = self.caches.clone();
        Box::pin(async move {
            let mut caches = caches.lock().unwrap();
            if let Some(existing) = caches.iter_mut().find(|existing| {
                existing.user_id == cache.user_id
                    && existing.provider == cache.provider
                    && existing.model == cache.model
                    && existing.scope_key == cache.scope_key
                    && existing.context_hash == cache.context_hash
            }) {
                *existing = cache.clone();
            } else {
                caches.push(cache.clone());
            }
            Ok(cache)
        })
    }

    fn delete_by_user_id(&self, user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
        let caches = self.caches.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            caches
                .lock()
                .unwrap()
                .retain(|cache| cache.user_id != user_id);
            Ok(())
        })
    }
}
