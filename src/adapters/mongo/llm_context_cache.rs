use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::llm::{
    BoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError, LlmProvider,
};

#[derive(Clone)]
pub struct MongoLlmContextCacheRepository {
    collection: Collection<LlmContextCacheDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LlmContextCacheDocument {
    user_id: String,
    provider: String,
    model: String,
    scope_key: String,
    context_hash: String,
    provider_cache_id: String,
    expires_at_epoch_seconds: Option<i64>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
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

            document.map(map_document_to_domain).transpose()
        })
    }

    fn upsert(&self, cache: LlmContextCache) -> BoxFuture<Result<LlmContextCache, LlmError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_domain_to_document(&cache);
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "provider": &document.provider,
                        "model": &document.model,
                        "scope_key": &document.scope_key,
                        "context_hash": &document.context_hash,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?;

            Ok(cache)
        })
    }

    fn delete_by_user_id(&self, user_id: &str) -> BoxFuture<Result<(), LlmError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .delete_many(doc! { "user_id": &user_id })
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_domain_to_document(cache: &LlmContextCache) -> LlmContextCacheDocument {
    LlmContextCacheDocument {
        user_id: cache.user_id.clone(),
        provider: cache.provider.as_str().to_string(),
        model: cache.model.clone(),
        scope_key: cache.scope_key.clone(),
        context_hash: cache.context_hash.clone(),
        provider_cache_id: cache.provider_cache_id.clone(),
        expires_at_epoch_seconds: cache.expires_at_epoch_seconds,
        created_at_epoch_seconds: cache.created_at_epoch_seconds,
        updated_at_epoch_seconds: cache.updated_at_epoch_seconds,
    }
}

fn map_document_to_domain(document: LlmContextCacheDocument) -> Result<LlmContextCache, LlmError> {
    Ok(LlmContextCache {
        user_id: document.user_id,
        provider: LlmProvider::parse(&document.provider).ok_or_else(|| {
            LlmError::Internal(format!(
                "unknown llm provider in context cache: {}",
                document.provider
            ))
        })?,
        model: document.model,
        scope_key: document.scope_key,
        context_hash: document.context_hash,
        provider_cache_id: document.provider_cache_id,
        expires_at_epoch_seconds: document.expires_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
