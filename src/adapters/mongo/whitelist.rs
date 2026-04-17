use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::identity::{BoxFuture, IdentityError, WhitelistEntry, WhitelistRepository};

#[derive(Clone)]
pub struct MongoWhitelistRepository {
    collection: Collection<WhitelistEntryDocument>,
}

impl MongoWhitelistRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("login_whitelist"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IdentityError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "email_normalized": 1 })
                .options(
                    IndexOptions::builder()
                        .name("login_whitelist_email_normalized_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl WhitelistRepository for MongoWhitelistRepository {
    fn find_by_normalized_email(
        &self,
        normalized_email: &str,
    ) -> BoxFuture<Result<Option<WhitelistEntry>, IdentityError>> {
        let collection = self.collection.clone();
        let normalized_email = normalized_email.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "email_normalized": &normalized_email })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_whitelist_entry_document))
        })
    }

    fn save(&self, entry: WhitelistEntry) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        let collection = self.collection.clone();
        let document = WhitelistEntryDocument::from_entry(&entry);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! { "email_normalized": &document.email_normalized },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(entry)
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WhitelistEntryDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    email: String,
    email_normalized: String,
    allowed: bool,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl WhitelistEntryDocument {
    fn from_entry(entry: &WhitelistEntry) -> Self {
        Self {
            id: None,
            email: entry.email.clone(),
            email_normalized: entry.email_normalized.clone(),
            allowed: entry.allowed,
            created_at_epoch_seconds: entry.created_at_epoch_seconds,
            updated_at_epoch_seconds: entry.updated_at_epoch_seconds,
        }
    }
}

fn map_whitelist_entry_document(document: WhitelistEntryDocument) -> WhitelistEntry {
    WhitelistEntry {
        email: document.email,
        email_normalized: document.email_normalized,
        allowed: document.allowed,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    }
}
