use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::athlete_summary::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryRepository, BoxFuture,
};

#[derive(Clone)]
pub struct MongoAthleteSummaryRepository {
    collection: Collection<AthleteSummaryDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AthleteSummaryDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    user_id: String,
    summary_text: String,
    generated_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
    provider: Option<String>,
    model: Option<String>,
}

impl MongoAthleteSummaryRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("athlete_summary"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AthleteSummaryError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("athlete_summary_user_id_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl AthleteSummaryRepository for MongoAthleteSummaryRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<AthleteSummary>, AthleteSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id })
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
            Ok(document.map(map_document_to_domain))
        })
    }

    fn upsert(
        &self,
        summary: AthleteSummary,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        let collection = self.collection.clone();
        let user_id = summary.user_id.clone();
        let document = map_domain_to_document(&summary);
        Box::pin(async move {
            collection
                .replace_one(doc! { "user_id": &user_id }, document)
                .upsert(true)
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
            Ok(summary)
        })
    }
}

fn map_document_to_domain(document: AthleteSummaryDocument) -> AthleteSummary {
    AthleteSummary {
        user_id: document.user_id,
        summary_text: document.summary_text,
        generated_at_epoch_seconds: document.generated_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        provider: document.provider,
        model: document.model,
    }
}

fn map_domain_to_document(summary: &AthleteSummary) -> AthleteSummaryDocument {
    AthleteSummaryDocument {
        id: None,
        user_id: summary.user_id.clone(),
        summary_text: summary.summary_text.clone(),
        generated_at_epoch_seconds: summary.generated_at_epoch_seconds,
        created_at_epoch_seconds: summary.created_at_epoch_seconds,
        updated_at_epoch_seconds: summary.updated_at_epoch_seconds,
        provider: summary.provider.clone(),
        model: summary.model.clone(),
    }
}
