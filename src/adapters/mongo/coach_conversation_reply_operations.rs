use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::{
    error::is_duplicate_key_error,
    time::{optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds},
};
use crate::domain::coach_conversation::{
    BoxFuture, CoachConversationError, CoachConversationReplyClaimResult,
    CoachConversationReplyOperation, CoachConversationReplyOperationFailureKind,
    CoachConversationReplyOperationRepository, CoachConversationReplyOperationStatus,
};
use crate::domain::llm::{
    provider_transcript_from_legacy_response, LlmChatMessage, LlmFinishReason,
};

#[derive(Clone)]
pub struct MongoCoachConversationReplyOperationRepository {
    collection: Collection<CoachConversationReplyOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CoachConversationReplyOperationDocument {
    user_id: String,
    conversation_id: String,
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
    #[serde(default)]
    provider_transcript: Vec<LlmChatMessage>,
    #[serde(default)]
    response_message: Option<String>,
    #[serde(default)]
    finish_reason: Option<LlmFinishReason>,
    #[serde(default)]
    public_tool_call_ids: Vec<String>,
    error_message: Option<String>,
    started_at_epoch_seconds: i64,
    #[serde(default)]
    started_at: Option<DateTime>,
    last_attempt_at_epoch_seconds: i64,
    #[serde(default)]
    last_attempt_at: Option<DateTime>,
    attempt_count: i64,
    created_at_epoch_seconds: i64,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: i64,
    #[serde(default)]
    updated_at: Option<DateTime>,
}

impl MongoCoachConversationReplyOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("coach_conversation_reply_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CoachConversationError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1, "conversation_id": 1, "user_message_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name(
                            "coach_conversation_reply_operations_user_conversation_message_unique"
                                .to_string(),
                        )
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}

impl CoachConversationReplyOperationRepository for MongoCoachConversationReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversationReplyOperation>, CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "conversation_id": &conversation_id,
                    "user_message_id": &user_message_id,
                })
                .await
                .map_err(storage_error)?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: CoachConversationReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachConversationReplyClaimResult, CoachConversationError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation);
            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(storage_error(error))
                    }
                })?;

            if inserted {
                return Ok(CoachConversationReplyClaimResult::Claimed(operation));
            }

            let existing_document = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "conversation_id": &document.conversation_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    CoachConversationError::Repository(
                        "claimed coach conversation reply operation disappeared before reload"
                            .to_string(),
                    )
                })?;

            let existing = map_document_to_operation(existing_document)?;
            let reclaimable = match existing.status {
                CoachConversationReplyOperationStatus::Pending => {
                    existing.is_stale(stale_before_epoch_seconds)
                }
                CoachConversationReplyOperationStatus::Failed => true,
                CoachConversationReplyOperationStatus::Completed => false,
            };

            if !reclaimable {
                return Ok(CoachConversationReplyClaimResult::Existing(existing));
            }

            let fallback_coach_message_id =
                operation.coach_message_id.clone().ok_or_else(|| {
                    CoachConversationError::Repository(
                    "pending coach conversation reply operation missing reserved coach message id"
                        .to_string(),
                )
                })?;
            let reclaimed = existing.reclaim(
                fallback_coach_message_id,
                operation.last_attempt_at_epoch_seconds,
            );
            let reclaimed_document = map_operation_to_document(&reclaimed);
            let replaced = collection
                .find_one_and_replace(
                    doc! {
                        "user_id": &document.user_id,
                        "conversation_id": &document.conversation_id,
                        "user_message_id": &document.user_message_id,
                        "attempt_count": i64::from(existing.attempt_count),
                        "updated_at_epoch_seconds": existing.updated_at_epoch_seconds,
                    },
                    &reclaimed_document,
                )
                .await
                .map_err(storage_error)?;

            if replaced.is_some() {
                return Ok(CoachConversationReplyClaimResult::Claimed(reclaimed));
            }

            let latest = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "conversation_id": &document.conversation_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    CoachConversationError::Repository(
                        "reclaimed coach conversation reply operation disappeared before reload"
                            .to_string(),
                    )
                })?;

            Ok(CoachConversationReplyClaimResult::Existing(
                map_document_to_operation(latest)?,
            ))
        })
    }

    fn upsert(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> BoxFuture<Result<CoachConversationReplyOperation, CoachConversationError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation);
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "conversation_id": &document.conversation_id,
                        "user_message_id": &document.user_message_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(storage_error)?;
            Ok(operation)
        })
    }
}

fn map_operation_to_document(
    operation: &CoachConversationReplyOperation,
) -> CoachConversationReplyOperationDocument {
    CoachConversationReplyOperationDocument {
        user_id: operation.user_id.clone(),
        conversation_id: operation.conversation_id.clone(),
        user_message_id: operation.user_message_id.clone(),
        status: status_as_str(&operation.status).to_string(),
        failure_kind: operation
            .failure_kind
            .as_ref()
            .map(failure_kind_as_str)
            .map(str::to_string),
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
        provider_transcript: operation.provider_transcript.clone(),
        response_message: None,
        finish_reason: operation.finish_reason.clone(),
        public_tool_call_ids: operation.public_tool_call_ids.clone(),
        error_message: operation.error_message.clone(),
        started_at_epoch_seconds: operation.started_at_epoch_seconds,
        started_at: optional_epoch_seconds_to_bson_datetime(
            Some(operation.started_at_epoch_seconds),
            "started_at",
        )
        .expect("started_at should fit BSON DateTime"),
        last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
        last_attempt_at: optional_epoch_seconds_to_bson_datetime(
            Some(operation.last_attempt_at_epoch_seconds),
            "last_attempt_at",
        )
        .expect("last_attempt_at should fit BSON DateTime"),
        attempt_count: i64::from(operation.attempt_count),
        created_at_epoch_seconds: operation.created_at_epoch_seconds,
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(operation.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(operation.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
    }
}

fn map_document_to_operation(
    document: CoachConversationReplyOperationDocument,
) -> Result<CoachConversationReplyOperation, CoachConversationError> {
    Ok(CoachConversationReplyOperation {
        user_id: document.user_id,
        conversation_id: document.conversation_id,
        user_message_id: document.user_message_id,
        status: map_status(document.status)?,
        failure_kind: document.failure_kind.map(map_failure_kind).transpose()?,
        provider: document
            .provider
            .map(|value| {
                crate::domain::llm::LlmProvider::parse(&value).ok_or_else(|| {
                    CoachConversationError::Repository(format!(
                        "unknown llm provider in coach conversation reply operation: {value}"
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
        provider_transcript: provider_transcript_from_legacy_response(
            document.provider_transcript,
            document.response_message,
        ),
        finish_reason: document.finish_reason,
        public_tool_call_ids: document.public_tool_call_ids,
        error_message: document.error_message,
        started_at_epoch_seconds: resolve_required_epoch_seconds(
            document.started_at,
            Some(document.started_at_epoch_seconds),
            "started_at",
        )
        .map_err(CoachConversationError::Repository)?,
        last_attempt_at_epoch_seconds: resolve_required_epoch_seconds(
            document.last_attempt_at,
            Some(document.last_attempt_at_epoch_seconds),
            "last_attempt_at",
        )
        .map_err(CoachConversationError::Repository)?,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            CoachConversationError::Repository(
                "invalid coach conversation reply attempt count".to_string(),
            )
        })?,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            Some(document.created_at_epoch_seconds),
            "created_at",
        )
        .map_err(CoachConversationError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            Some(document.updated_at_epoch_seconds),
            "updated_at",
        )
        .map_err(CoachConversationError::Repository)?,
    })
}

fn status_as_str(status: &CoachConversationReplyOperationStatus) -> &'static str {
    match status {
        CoachConversationReplyOperationStatus::Pending => "pending",
        CoachConversationReplyOperationStatus::Completed => "completed",
        CoachConversationReplyOperationStatus::Failed => "failed",
    }
}

fn map_status(
    value: String,
) -> Result<CoachConversationReplyOperationStatus, CoachConversationError> {
    match value.as_str() {
        "pending" => Ok(CoachConversationReplyOperationStatus::Pending),
        "completed" => Ok(CoachConversationReplyOperationStatus::Completed),
        "failed" => Ok(CoachConversationReplyOperationStatus::Failed),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation reply operation status: {other}"
        ))),
    }
}

fn failure_kind_as_str(failure_kind: &CoachConversationReplyOperationFailureKind) -> &'static str {
    match failure_kind {
        CoachConversationReplyOperationFailureKind::CredentialsNotConfigured => {
            "credentials_not_configured"
        }
        CoachConversationReplyOperationFailureKind::ProviderNotConfigured => {
            "provider_not_configured"
        }
        CoachConversationReplyOperationFailureKind::ModelNotConfigured => "model_not_configured",
        CoachConversationReplyOperationFailureKind::ContextTooLarge => "context_too_large",
        CoachConversationReplyOperationFailureKind::UnsupportedProvider => "unsupported_provider",
        CoachConversationReplyOperationFailureKind::Transport => "transport",
        CoachConversationReplyOperationFailureKind::ProviderRejected => "provider_rejected",
        CoachConversationReplyOperationFailureKind::RateLimited => "rate_limited",
        CoachConversationReplyOperationFailureKind::InvalidResponse => "invalid_response",
        CoachConversationReplyOperationFailureKind::Internal => "internal",
    }
}

fn map_failure_kind(
    value: String,
) -> Result<CoachConversationReplyOperationFailureKind, CoachConversationError> {
    match value.as_str() {
        "credentials_not_configured" => {
            Ok(CoachConversationReplyOperationFailureKind::CredentialsNotConfigured)
        }
        "provider_not_configured" => {
            Ok(CoachConversationReplyOperationFailureKind::ProviderNotConfigured)
        }
        "model_not_configured" => {
            Ok(CoachConversationReplyOperationFailureKind::ModelNotConfigured)
        }
        "context_too_large" => Ok(CoachConversationReplyOperationFailureKind::ContextTooLarge),
        "unsupported_provider" => {
            Ok(CoachConversationReplyOperationFailureKind::UnsupportedProvider)
        }
        "transport" => Ok(CoachConversationReplyOperationFailureKind::Transport),
        "provider_rejected" => Ok(CoachConversationReplyOperationFailureKind::ProviderRejected),
        "rate_limited" => Ok(CoachConversationReplyOperationFailureKind::RateLimited),
        "invalid_response" => Ok(CoachConversationReplyOperationFailureKind::InvalidResponse),
        "internal" => Ok(CoachConversationReplyOperationFailureKind::Internal),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation reply operation failure kind: {other}"
        ))),
    }
}

fn storage_error(error: mongodb::error::Error) -> CoachConversationError {
    CoachConversationError::Repository(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{map_document_to_operation, CoachConversationReplyOperationDocument};
    use crate::domain::coach_conversation::CoachConversationReplyOperationStatus;
    use crate::domain::llm::{LlmChatMessage, LlmToolCall};

    #[test]
    fn map_document_to_operation_reuses_legacy_response_message_when_provider_transcript_missing() {
        let operation = map_document_to_operation(CoachConversationReplyOperationDocument {
            user_id: "user-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            user_message_id: "message-1".to_string(),
            status: "pending".to_string(),
            failure_kind: None,
            provider: None,
            model: None,
            provider_request_id: None,
            coach_message_id: None,
            cache_scope_key: None,
            provider_cache_id: None,
            token_usage: None,
            cache_usage: None,
            provider_transcript: Vec::new(),
            response_message: Some("Legacy conversation checkpoint".to_string()),
            finish_reason: None,
            public_tool_call_ids: Vec::new(),
            error_message: None,
            started_at_epoch_seconds: 1,
            started_at: None,
            last_attempt_at_epoch_seconds: 2,
            last_attempt_at: None,
            attempt_count: 1,
            created_at_epoch_seconds: 3,
            created_at: None,
            updated_at_epoch_seconds: 4,
            updated_at: None,
        })
        .expect("legacy response_message should map");

        assert_eq!(
            operation.status,
            CoachConversationReplyOperationStatus::Pending
        );
        assert_eq!(
            operation.provider_transcript,
            vec![LlmChatMessage::assistant("Legacy conversation checkpoint")]
        );
    }

    #[test]
    fn map_document_to_operation_preserves_full_tool_call_provider_transcript() {
        let operation = map_document_to_operation(CoachConversationReplyOperationDocument {
            user_id: "user-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            user_message_id: "message-1".to_string(),
            status: "pending".to_string(),
            failure_kind: None,
            provider: None,
            model: None,
            provider_request_id: None,
            coach_message_id: None,
            cache_scope_key: None,
            provider_cache_id: None,
            token_usage: None,
            cache_usage: None,
            provider_transcript: vec![
                LlmChatMessage::assistant_with_tool_calls(
                    "",
                    vec![LlmToolCall {
                        id: "tool-call-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: "{\"date\":\"2026-05-04\"}".to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-call-1", "calendar result"),
            ],
            response_message: Some("Legacy conversation checkpoint".to_string()),
            finish_reason: None,
            public_tool_call_ids: vec!["tool-call-1".to_string()],
            error_message: None,
            started_at_epoch_seconds: 1,
            started_at: None,
            last_attempt_at_epoch_seconds: 2,
            last_attempt_at: None,
            attempt_count: 1,
            created_at_epoch_seconds: 3,
            created_at: None,
            updated_at_epoch_seconds: 4,
            updated_at: None,
        })
        .expect("full tool-call transcript should map");

        assert_eq!(
            operation.status,
            CoachConversationReplyOperationStatus::Pending
        );
        assert_eq!(
            operation.provider_transcript,
            vec![
                LlmChatMessage::assistant_with_tool_calls(
                    "",
                    vec![LlmToolCall {
                        id: "tool-call-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: "{\"date\":\"2026-05-04\"}".to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-call-1", "calendar result"),
            ]
        );
        assert_eq!(
            operation.public_tool_call_ids,
            vec!["tool-call-1".to_string()]
        );
    }
}
