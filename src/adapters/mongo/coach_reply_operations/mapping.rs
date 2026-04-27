use crate::domain::workout_summary::{
    CoachReplyOperation, CoachReplyOperationFailureKind, CoachReplyOperationStatus,
    WorkoutSummaryError,
};

use super::document::CoachReplyOperationDocument;

fn map_failure_kind_to_document(failure_kind: &CoachReplyOperationFailureKind) -> String {
    match failure_kind {
        CoachReplyOperationFailureKind::CredentialsNotConfigured => "credentials_not_configured",
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

pub(super) fn map_operation_to_document(
    operation: &CoachReplyOperation,
) -> CoachReplyOperationDocument {
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
            .map(map_failure_kind_to_document),
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

pub(super) fn map_document_to_operation(
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
            .map(map_document_to_failure_kind)
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
