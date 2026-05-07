use crate::adapters::mongo::time::{
    optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds,
};
use crate::domain::llm::{
    provider_transcript_from_legacy_response, LlmReplyOperation, LlmReplyOperationFailureKind,
    LlmReplyOperationStatus,
};

use super::document::LlmReplyOperationDocument;

pub(super) fn map_operation_to_document(
    operation: &LlmReplyOperation,
    scope_type: &str,
) -> LlmReplyOperationDocument {
    LlmReplyOperationDocument {
        user_id: operation.user_id.clone(),
        scope_id: operation.scope_id.clone(),
        scope_type: scope_type.to_string(),
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
        // Domain `reply_message_id` is persisted under the legacy `coach_message_id` field name.
        coach_message_id: operation.reply_message_id.clone(),
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

pub(super) fn map_document_to_operation(
    document: LlmReplyOperationDocument,
) -> Result<LlmReplyOperation, String> {
    Ok(LlmReplyOperation {
        user_id: document.user_id,
        scope_id: document.scope_id,
        user_message_id: document.user_message_id,
        status: map_status(document.status)?,
        failure_kind: document.failure_kind.map(map_failure_kind).transpose()?,
        provider: document
            .provider
            .map(|value| {
                crate::domain::llm::LlmProvider::parse(&value)
                    .ok_or_else(|| format!("unknown llm provider in reply operation: {value}"))
            })
            .transpose()?,
        model: document.model,
        provider_request_id: document.provider_request_id,
        reply_message_id: document.coach_message_id,
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
        )?,
        last_attempt_at_epoch_seconds: resolve_required_epoch_seconds(
            document.last_attempt_at,
            Some(document.last_attempt_at_epoch_seconds),
            "last_attempt_at",
        )?,
        attempt_count: u32::try_from(document.attempt_count)
            .map_err(|_| "invalid reply attempt count".to_string())?,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            Some(document.created_at_epoch_seconds),
            "created_at",
        )?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            Some(document.updated_at_epoch_seconds),
            "updated_at",
        )?,
    })
}

fn status_as_str(status: &LlmReplyOperationStatus) -> &'static str {
    match status {
        LlmReplyOperationStatus::Pending => "pending",
        LlmReplyOperationStatus::Completed => "completed",
        LlmReplyOperationStatus::Failed => "failed",
    }
}

fn map_status(value: String) -> Result<LlmReplyOperationStatus, String> {
    match value.as_str() {
        "pending" => Ok(LlmReplyOperationStatus::Pending),
        "completed" => Ok(LlmReplyOperationStatus::Completed),
        "failed" => Ok(LlmReplyOperationStatus::Failed),
        other => Err(format!("unknown reply operation status: {other}")),
    }
}

fn failure_kind_as_str(failure_kind: &LlmReplyOperationFailureKind) -> &'static str {
    match failure_kind {
        LlmReplyOperationFailureKind::CredentialsNotConfigured => "credentials_not_configured",
        LlmReplyOperationFailureKind::ProviderNotConfigured => "provider_not_configured",
        LlmReplyOperationFailureKind::ModelNotConfigured => "model_not_configured",
        LlmReplyOperationFailureKind::ContextTooLarge => "context_too_large",
        LlmReplyOperationFailureKind::UnsupportedProvider => "unsupported_provider",
        LlmReplyOperationFailureKind::Transport => "transport",
        LlmReplyOperationFailureKind::ProviderRejected => "provider_rejected",
        LlmReplyOperationFailureKind::RateLimited => "rate_limited",
        LlmReplyOperationFailureKind::InvalidResponse => "invalid_response",
        LlmReplyOperationFailureKind::Checkpoint => "checkpoint",
        LlmReplyOperationFailureKind::Internal => "internal",
    }
}

fn map_failure_kind(value: String) -> Result<LlmReplyOperationFailureKind, String> {
    match value.as_str() {
        "credentials_not_configured" => Ok(LlmReplyOperationFailureKind::CredentialsNotConfigured),
        "provider_not_configured" => Ok(LlmReplyOperationFailureKind::ProviderNotConfigured),
        "model_not_configured" => Ok(LlmReplyOperationFailureKind::ModelNotConfigured),
        "context_too_large" => Ok(LlmReplyOperationFailureKind::ContextTooLarge),
        "unsupported_provider" => Ok(LlmReplyOperationFailureKind::UnsupportedProvider),
        "transport" => Ok(LlmReplyOperationFailureKind::Transport),
        "provider_rejected" => Ok(LlmReplyOperationFailureKind::ProviderRejected),
        "rate_limited" => Ok(LlmReplyOperationFailureKind::RateLimited),
        "invalid_response" => Ok(LlmReplyOperationFailureKind::InvalidResponse),
        "checkpoint" => Ok(LlmReplyOperationFailureKind::Checkpoint),
        "internal" => Ok(LlmReplyOperationFailureKind::Internal),
        other => Err(format!("unknown reply operation failure kind: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::document::LlmReplyOperationDocument;
    use super::map_document_to_operation;
    use crate::domain::llm::{LlmChatMessage, LlmReplyOperationStatus, LlmToolCall};

    #[test]
    fn map_document_to_operation_reuses_legacy_response_message_when_provider_transcript_missing() {
        let operation = map_document_to_operation(LlmReplyOperationDocument {
            user_id: "user-1".to_string(),
            scope_id: "scope-1".to_string(),
            scope_type: "workout_summary".to_string(),
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
            response_message: Some("Legacy checkpoint".to_string()),
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

        assert_eq!(operation.status, LlmReplyOperationStatus::Pending);
        assert_eq!(
            operation.provider_transcript,
            vec![LlmChatMessage::assistant("Legacy checkpoint")]
        );
    }

    #[test]
    fn map_document_to_operation_preserves_full_tool_call_provider_transcript() {
        let operation = map_document_to_operation(LlmReplyOperationDocument {
            user_id: "user-1".to_string(),
            scope_id: "scope-1".to_string(),
            scope_type: "coach_conversation".to_string(),
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

        assert_eq!(operation.status, LlmReplyOperationStatus::Pending);
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
