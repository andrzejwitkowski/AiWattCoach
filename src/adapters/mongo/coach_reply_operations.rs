use super::error::is_duplicate_key_error;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::workout_summary::{
    BoxFuture, CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationFailureKind,
    CoachReplyOperationRepository, CoachReplyOperationStatus, WorkoutSummaryError,
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
    failure_kind: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    provider_request_id: Option<String>,
    coach_message_id: Option<String>,
    cache_scope_key: Option<String>,
    provider_cache_id: Option<String>,
    token_usage: Option<crate::domain::llm::LlmTokenUsage>,
    cache_usage: Option<crate::domain::llm::LlmCacheUsage>,
    response_message: Option<String>,
    error_message: Option<String>,
    started_at_epoch_seconds: i64,
    last_attempt_at_epoch_seconds: i64,
    attempt_count: i64,
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

    fn map_failure_kind_to_document(failure_kind: &CoachReplyOperationFailureKind) -> String {
        match failure_kind {
            CoachReplyOperationFailureKind::CredentialsNotConfigured => {
                "credentials_not_configured"
            }
            CoachReplyOperationFailureKind::ProviderNotConfigured => "provider_not_configured",
            CoachReplyOperationFailureKind::ModelNotConfigured => "model_not_configured",
            CoachReplyOperationFailureKind::ContextTooLarge => "context_too_large",
            CoachReplyOperationFailureKind::UnsupportedProvider => "unsupported_provider",
            CoachReplyOperationFailureKind::Transport => "transport",
            CoachReplyOperationFailureKind::ProviderRejected => "provider_rejected",
            CoachReplyOperationFailureKind::RateLimited => "rate_limited",
            CoachReplyOperationFailureKind::InvalidResponse => "invalid_response",
            CoachReplyOperationFailureKind::Internal => "internal",
        }
        .to_string()
    }

    fn map_document_to_failure_kind(
        value: String,
    ) -> Result<CoachReplyOperationFailureKind, WorkoutSummaryError> {
        match value.as_str() {
            "credentials_not_configured" => {
                Ok(CoachReplyOperationFailureKind::CredentialsNotConfigured)
            }
            "provider_not_configured" => Ok(CoachReplyOperationFailureKind::ProviderNotConfigured),
            "model_not_configured" => Ok(CoachReplyOperationFailureKind::ModelNotConfigured),
            "context_too_large" => Ok(CoachReplyOperationFailureKind::ContextTooLarge),
            "unsupported_provider" => Ok(CoachReplyOperationFailureKind::UnsupportedProvider),
            "transport" => Ok(CoachReplyOperationFailureKind::Transport),
            "provider_rejected" => Ok(CoachReplyOperationFailureKind::ProviderRejected),
            "rate_limited" => Ok(CoachReplyOperationFailureKind::RateLimited),
            "invalid_response" => Ok(CoachReplyOperationFailureKind::InvalidResponse),
            "internal" => Ok(CoachReplyOperationFailureKind::Internal),
            other => Err(WorkoutSummaryError::Repository(format!(
                "unknown coach reply operation failure kind: {other}"
            ))),
        }
    }

    fn map_operation_to_document(operation: &CoachReplyOperation) -> CoachReplyOperationDocument {
        CoachReplyOperationDocument {
            user_id: operation.user_id.clone(),
            workout_id: operation.workout_id.clone(),
            user_message_id: operation.user_message_id.clone(),
            status: match operation.status {
                CoachReplyOperationStatus::Pending => "pending",
                CoachReplyOperationStatus::Completed => "completed",
                CoachReplyOperationStatus::Failed => "failed",
            }
            .to_string(),
            failure_kind: operation
                .failure_kind
                .as_ref()
                .map(Self::map_failure_kind_to_document),
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
            response_message: operation.response_message.clone(),
            error_message: operation.error_message.clone(),
            started_at_epoch_seconds: operation.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
            attempt_count: i64::from(operation.attempt_count),
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
                "pending" => CoachReplyOperationStatus::Pending,
                "completed" => CoachReplyOperationStatus::Completed,
                "failed" => CoachReplyOperationStatus::Failed,
                other => {
                    return Err(WorkoutSummaryError::Repository(format!(
                        "unknown coach reply operation status: {other}"
                    )))
                }
            },
            failure_kind: document
                .failure_kind
                .map(Self::map_document_to_failure_kind)
                .transpose()?,
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
            response_message: document.response_message,
            error_message: document.error_message,
            started_at_epoch_seconds: document.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: document.last_attempt_at_epoch_seconds,
            attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
                WorkoutSummaryError::Repository("invalid coach reply attempt count".to_string())
            })?,
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
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = Self::map_operation_to_document(&operation);

            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(WorkoutSummaryError::Repository(error.to_string()))
                    }
                })?;

            if inserted {
                return Ok(CoachReplyClaimResult::Claimed(operation));
            }

            let existing_document = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "workout_id": &document.workout_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "claimed coach reply operation disappeared before reload".to_string(),
                    )
                })?;

            let existing = Self::map_document_to_operation(existing_document)?;
            let reclaimable = match existing.status {
                CoachReplyOperationStatus::Pending => existing.is_stale(stale_before_epoch_seconds),
                CoachReplyOperationStatus::Failed => true,
                CoachReplyOperationStatus::Completed => false,
            };

            if !reclaimable {
                return Ok(CoachReplyClaimResult::Existing(existing));
            }

            let fallback_coach_message_id =
                operation.coach_message_id.clone().ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "pending coach reply operation missing reserved coach message id"
                            .to_string(),
                    )
                })?;
            let reclaimed = existing.reclaim(
                fallback_coach_message_id,
                operation.last_attempt_at_epoch_seconds,
            );
            let reclaimed_document = Self::map_operation_to_document(&reclaimed);
            let replaced = collection
                .find_one_and_replace(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                        "attempt_count": i64::from(existing.attempt_count),
                        "updated_at_epoch_seconds": existing.updated_at_epoch_seconds,
                    },
                    &reclaimed_document,
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if replaced.is_some() {
                return Ok(CoachReplyClaimResult::Claimed(reclaimed));
            }

            let latest = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "workout_id": &document.workout_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "reclaimed coach reply operation disappeared before reload".to_string(),
                    )
                })?;

            Ok(CoachReplyClaimResult::Existing(
                Self::map_document_to_operation(latest)?,
            ))
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
