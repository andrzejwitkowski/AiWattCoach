use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};

mod document;
mod mapping;
mod repository;

use document::LlmReplyOperationDocument;

#[derive(Clone)]
pub struct MongoLlmReplyOperationRepository {
    collection: Collection<LlmReplyOperationDocument>,
    scope_type: &'static str,
}

impl MongoLlmReplyOperationRepository {
    pub fn new(
        client: mongodb::Client,
        database: impl AsRef<str>,
        scope_type: &'static str,
    ) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("llm_reply_operations"),
            scope_type,
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), mongodb::error::Error> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! {
                    "user_id": 1,
                    "scope_type": 1,
                    "scope_id": 1,
                    "user_message_id": 1,
                })
                .options(
                    IndexOptions::builder()
                        .name("llm_reply_operations_user_scope_message_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await?;
        Ok(())
    }
}
