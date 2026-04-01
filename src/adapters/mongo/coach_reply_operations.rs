use mongodb::{
    bson::doc,
    options::{IndexOptions, ReturnDocument},
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::workout_summary::{
    BoxFuture, CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository,
    CoachReplyOperationStatus, WorkoutSummaryError,
};

#[derive(Clone)]
pub struct MongoCoachReplyOperationRepository {
    collection: Collection<CoachReplyOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CoachReplyOperationDocument {
    user_id: String,
    workout_id: String,
    user_message_id: String,
    status: String,
    provider: Option<String>,
    model: Option<String>,
    provider_request_id: Option<String>,
    coach_message_id: Option<String>,
    cache_scope_key: Option<String>,
    provider_cache_id: Option<String>,
    token_usage: Option<crate::domain::llm::LlmTokenUsage>,
    cache_usage: Option<crate::domain::llm::LlmCacheUsage>,
    error_message: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoCoachReplyOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("coach_reply_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WorkoutSummaryError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1, "workout_id": 1, "user_message_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("coach_reply_operations_user_workout_message_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
        Ok(())
    }

    fn map_operation_to_document(operation: &CoachReplyOperation) -> CoachReplyOperationDocument {
        CoachReplyOperationDocument {
            user_id: operation.user_id.clone(),
            workout_id: operation.workout_id.clone(),
            user_message_id: operation.user_message_id.clone(),
            status: match operation.status {
                crate::domain::workout_summary::CoachReplyOperationStatus::Pending => "pending",
                crate::domain::workout_summary::CoachReplyOperationStatus::Completed => "completed",
                crate::domain::workout_summary::CoachReplyOperationStatus::Failed => "failed",
            }
            .to_string(),
            provider: operation
                .provider
                .as_ref()
                .map(|provider| provider.as_str().to_string()),
            model: operation.model.clone(),
            provider_request_id: operation.provider_request_id.clone(),
            coach_message_id: operation.coach_message_id.clone(),
            cache_scope_key: operation.cache_scope_key.clone(),
            provider_cache_id: operation.provider_cache_id.clone(),
            token_usage: operation.token_usage.clone(),
            cache_usage: operation.cache_usage.clone(),
            error_message: operation.error_message.clone(),
            created_at_epoch_seconds: operation.created_at_epoch_seconds,
            updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
        }
    }

    fn map_document_to_operation(
        document: CoachReplyOperationDocument,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        Ok(CoachReplyOperation {
            user_id: document.user_id,
            workout_id: document.workout_id,
            user_message_id: document.user_message_id,
            status: match document.status.as_str() {
                "pending" => crate::domain::workout_summary::CoachReplyOperationStatus::Pending,
                "completed" => crate::domain::workout_summary::CoachReplyOperationStatus::Completed,
                "failed" => crate::domain::workout_summary::CoachReplyOperationStatus::Failed,
                other => {
                    return Err(WorkoutSummaryError::Repository(format!(
                        "unknown coach reply operation status: {other}"
                    )))
                }
            },
            provider: document
                .provider
                .map(|value| {
                    crate::domain::llm::LlmProvider::parse(&value).ok_or_else(|| {
                        WorkoutSummaryError::Repository(format!(
                            "unknown llm provider in coach reply operation: {value}"
                        ))
                    })
                })
                .transpose()?,
            model: document.model,
            provider_request_id: document.provider_request_id,
            coach_message_id: document.coach_message_id,
            cache_scope_key: document.cache_scope_key,
            provider_cache_id: document.provider_cache_id,
            token_usage: document.token_usage,
            cache_usage: document.cache_usage,
            error_message: document.error_message,
            created_at_epoch_seconds: document.created_at_epoch_seconds,
            updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        })
    }
}

impl CoachReplyOperationRepository for MongoCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "workout_id": &workout_id,
                    "user_message_id": &user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            document.map(Self::map_document_to_operation).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = Self::map_operation_to_document(&operation);
            let operation_document = mongodb::bson::to_document(&document)
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            let existing = collection
                .find_one_and_update(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                    },
                    doc! {
                        "$setOnInsert": operation_document.clone(),
                    },
                )
                .upsert(true)
                .return_document(ReturnDocument::Before)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            match existing {
                Some(document) => {
                    let existing = Self::map_document_to_operation(document)?;
                    if existing.status == CoachReplyOperationStatus::Failed {
                        collection
                            .replace_one(
                                doc! {
                                    "user_id": &operation.user_id,
                                    "workout_id": &operation.workout_id,
                                    "user_message_id": &operation.user_message_id,
                                    "status": "failed",
                                },
                                Self::map_operation_to_document(&operation),
                            )
                            .await
                            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
                        Ok(CoachReplyClaimResult::Claimed(operation))
                    } else {
                        Ok(CoachReplyClaimResult::Existing(existing))
                    }
                }
                None => Ok(CoachReplyClaimResult::Claimed(operation)),
            }
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = Self::map_operation_to_document(&operation);

            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            Ok(operation)
        })
    }
}
