use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds};
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
    generated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    generated_at: Option<DateTime>,
    created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    updated_at: Option<DateTime>,
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
        generated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.generated_at,
            document.generated_at_epoch_seconds,
            "generated_at",
        )
        .expect("athlete summary documents must store generated_at"),
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .expect("athlete summary documents must store created_at"),
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .expect("athlete summary documents must store updated_at"),
        provider: document.provider,
        model: document.model,
    }
}

fn map_domain_to_document(summary: &AthleteSummary) -> AthleteSummaryDocument {
    AthleteSummaryDocument {
        id: None,
        user_id: summary.user_id.clone(),
        summary_text: summary.summary_text.clone(),
        generated_at_epoch_seconds: Some(summary.generated_at_epoch_seconds),
        generated_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.generated_at_epoch_seconds),
            "generated_at",
        )
        .expect("generated_at should fit BSON DateTime"),
        created_at_epoch_seconds: Some(summary.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: Some(summary.updated_at_epoch_seconds),
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
        provider: summary.provider.clone(),
        model: summary.model.clone(),
    }
}
