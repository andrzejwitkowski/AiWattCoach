use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::error::is_duplicate_key_error;
use crate::domain::athlete_summary::{
    AthleteSummaryError, AthleteSummaryGenerationClaimResult, AthleteSummaryGenerationOperation,
    AthleteSummaryGenerationOperationRepository, AthleteSummaryGenerationOperationStatus,
    BoxFuture,
};

#[derive(Clone)]
pub struct MongoAthleteSummaryGenerationOperationRepository {
    collection: Collection<AthleteSummaryGenerationOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AthleteSummaryGenerationOperationDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    user_id: String,
    status: String,
    summary_text: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    error_message: Option<String>,
    started_at_epoch_seconds: i64,
    last_attempt_at_epoch_seconds: i64,
    attempt_count: i64,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoAthleteSummaryGenerationOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("athlete_summary_generation_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), AthleteSummaryError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("athlete_summary_generation_operations_user_id_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
        Ok(())
    }

    fn reclaim_operation(
        existing: &AthleteSummaryGenerationOperation,
        pending: &AthleteSummaryGenerationOperation,
    ) -> AthleteSummaryGenerationOperation {
        AthleteSummaryGenerationOperation {
            user_id: existing.user_id.clone(),
            status: AthleteSummaryGenerationOperationStatus::Pending,
            summary_text: existing.summary_text.clone(),
            provider: existing.provider.clone(),
            model: existing.model.clone(),
            error_message: None,
            started_at_epoch_seconds: existing.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: pending.last_attempt_at_epoch_seconds,
            attempt_count: existing.attempt_count.saturating_add(1),
            created_at_epoch_seconds: existing.created_at_epoch_seconds,
            updated_at_epoch_seconds: pending.updated_at_epoch_seconds,
        }
    }
}

impl AthleteSummaryGenerationOperationRepository
    for MongoAthleteSummaryGenerationOperationRepository
{
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<AthleteSummaryGenerationOperation>, AthleteSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id })
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: AthleteSummaryGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<AthleteSummaryGenerationClaimResult, AthleteSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_domain_to_document(&operation)?;

            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(AthleteSummaryError::Repository(error.to_string()))
                    }
                })?;

            if inserted {
                return Ok(AthleteSummaryGenerationClaimResult::Claimed(operation));
            }

            let existing_document = collection
                .find_one(doc! { "user_id": &document.user_id })
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    AthleteSummaryError::Repository(
                        "claimed athlete summary generation operation disappeared before reload"
                            .to_string(),
                    )
                })?;
            let existing = map_document_to_domain(existing_document)?;
            let reclaimable = match existing.status {
                AthleteSummaryGenerationOperationStatus::Pending => {
                    existing.last_attempt_at_epoch_seconds <= stale_before_epoch_seconds
                }
                AthleteSummaryGenerationOperationStatus::Failed => true,
                AthleteSummaryGenerationOperationStatus::Completed => false,
            };

            if !reclaimable {
                return Ok(AthleteSummaryGenerationClaimResult::Existing(existing));
            }

            let reclaimed = Self::reclaim_operation(&existing, &operation);
            let reclaimed_document = map_domain_to_document(&reclaimed)?;
            let replaced = collection
                .find_one_and_replace(
                    doc! {
                        "user_id": &document.user_id,
                        "attempt_count": i64::from(existing.attempt_count),
                        "updated_at_epoch_seconds": existing.updated_at_epoch_seconds,
                    },
                    &reclaimed_document,
                )
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;

            if replaced.is_some() {
                return Ok(AthleteSummaryGenerationClaimResult::Claimed(reclaimed));
            }

            let latest = collection
                .find_one(doc! { "user_id": &document.user_id })
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    AthleteSummaryError::Repository(
                        "reclaimed athlete summary generation operation disappeared before reload"
                            .to_string(),
                    )
                })?;

            Ok(AthleteSummaryGenerationClaimResult::Existing(
                map_document_to_domain(latest)?,
            ))
        })
    }

    fn upsert(
        &self,
        operation: AthleteSummaryGenerationOperation,
    ) -> BoxFuture<Result<AthleteSummaryGenerationOperation, AthleteSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_domain_to_document(&operation)?;
            collection
                .replace_one(doc! { "user_id": &document.user_id }, &document)
                .upsert(true)
                .await
                .map_err(|error| AthleteSummaryError::Repository(error.to_string()))?;
            Ok(operation)
        })
    }
}

fn map_domain_to_document(
    operation: &AthleteSummaryGenerationOperation,
) -> Result<AthleteSummaryGenerationOperationDocument, AthleteSummaryError> {
    Ok(AthleteSummaryGenerationOperationDocument {
        id: None,
        user_id: operation.user_id.clone(),
        status: match operation.status {
            AthleteSummaryGenerationOperationStatus::Pending => "pending",
            AthleteSummaryGenerationOperationStatus::Completed => "completed",
            AthleteSummaryGenerationOperationStatus::Failed => "failed",
        }
        .to_string(),
        summary_text: operation.summary_text.clone(),
        provider: operation.provider.clone(),
        model: operation.model.clone(),
        error_message: operation.error_message.clone(),
        started_at_epoch_seconds: operation.started_at_epoch_seconds,
        last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
        attempt_count: i64::from(operation.attempt_count),
        created_at_epoch_seconds: operation.created_at_epoch_seconds,
        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
    })
}

fn map_document_to_domain(
    document: AthleteSummaryGenerationOperationDocument,
) -> Result<AthleteSummaryGenerationOperation, AthleteSummaryError> {
    Ok(AthleteSummaryGenerationOperation {
        user_id: document.user_id,
        status: match document.status.as_str() {
            "pending" => AthleteSummaryGenerationOperationStatus::Pending,
            "completed" => AthleteSummaryGenerationOperationStatus::Completed,
            "failed" => AthleteSummaryGenerationOperationStatus::Failed,
            other => {
                return Err(AthleteSummaryError::Repository(format!(
                    "unknown athlete summary generation operation status: {other}"
                )))
            }
        },
        summary_text: document.summary_text,
        provider: document.provider,
        model: document.model,
        error_message: document.error_message,
        started_at_epoch_seconds: document.started_at_epoch_seconds,
        last_attempt_at_epoch_seconds: document.last_attempt_at_epoch_seconds,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            AthleteSummaryError::Repository(
                "invalid athlete summary generation operation attempt count".to_string(),
            )
        })?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
