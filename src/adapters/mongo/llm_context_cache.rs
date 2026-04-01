use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};

use crate::domain::llm::{
    BoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError, LlmProvider,
};

#[derive(Clone)]
pub struct MongoLlmContextCacheRepository {
    collection: Collection<LlmContextCache>,
}

impl MongoLlmContextCacheRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("llm_context_caches"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), LlmError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! {
                    "user_id": 1,
                    "provider": 1,
                    "model": 1,
                    "scope_key": 1,
                    "context_hash": 1,
                })
                .options(
                    IndexOptions::builder()
                        .name(
                            "llm_context_caches_user_provider_model_scope_hash_unique".to_string(),
                        )
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| LlmError::Internal(error.to_string()))?;
        Ok(())
    }
}

impl LlmContextCacheRepository for MongoLlmContextCacheRepository {
    fn find_reusable(
        &self,
        user_id: &str,
        provider: &LlmProvider,
        model: &str,
        scope_key: &str,
        context_hash: &str,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider.as_str().to_string();
        let model = model.to_string();
        let scope_key = scope_key.to_string();
        let context_hash = context_hash.to_string();

        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "model": &model,
                    "scope_key": &scope_key,
                    "context_hash": &context_hash,
                    "$or": [
                        { "expires_at_epoch_seconds": { "$exists": false } },
                        { "expires_at_epoch_seconds": mongodb::bson::Bson::Null },
                        { "expires_at_epoch_seconds": { "$gt": now_epoch_seconds } }
                    ]
                })
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?;

            Ok(document)
        })
    }

    fn upsert(&self, cache: LlmContextCache) -> BoxFuture<Result<LlmContextCache, LlmError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &cache.user_id,
                        "provider": cache.provider.as_str(),
                        "model": &cache.model,
                        "scope_key": &cache.scope_key,
                        "context_hash": &cache.context_hash,
                    },
                    &cache,
                )
                .upsert(true)
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?;

            Ok(cache)
        })
    }
}
