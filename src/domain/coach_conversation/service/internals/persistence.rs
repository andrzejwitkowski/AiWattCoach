use crate::domain::coach_conversation::CoachConversationReplyOperation;
use crate::domain::llm::{
    merge_provider_transcript_entries, next_provider_transcript_updated_at_epoch_seconds,
    persistence::{retry_persist, RetryConfig, RetryContext},
    LlmChatMessage, LlmError,
};

use super::super::*;

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    pub(in super::super) async fn merge_provider_transcript_with_retry(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
        write_label: &'static str,
    ) -> Result<(), CoachConversationError> {
        let service = self.clone();
        let user_id = conversation.user_id.clone();
        let conversation_id = conversation.conversation_id.clone();
        let provider_transcript = operation.provider_transcript.clone();
        let ctx = RetryContext {
            write_label,
            user_message_id: operation.user_message_id.clone(),
            scope_label: "conversation_id",
            scope_value: conversation_id.clone(),
            operation_status: None,
        };

        retry_persist(
            RetryConfig {
                max_attempts: POST_PROVIDER_WRITE_ATTEMPTS,
                backoff_base_ms: 25,
            },
            |e| matches!(e, CoachConversationError::Repository(_)),
            || {
                let svc = service.clone();
                let uid = user_id.clone();
                let cid = conversation_id.clone();
                let pt = provider_transcript.clone();
                Box::pin(async move {
                    let latest = svc
                        .conversations
                        .find_by_user_id_and_conversation_id(&uid, &cid)
                        .await?
                        .ok_or(CoachConversationError::NotFound)?;
                    let merged =
                        merge_provider_transcript_entries(latest.provider_transcript.clone(), &pt);
                    svc.replace_provider_transcript(&latest, merged).await
                })
            },
            &ctx,
        )
        .await
    }

    pub(in super::super) fn existing_llm_failure_to_error(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> CoachConversationError {
        if let Some(failure_kind) = operation.failure_kind {
            return CoachConversationError::Llm(failure_kind.to_llm_error(operation.error_message));
        }

        CoachConversationError::Llm(LlmError::Internal(
            operation
                .error_message
                .unwrap_or_else(|| "failed coach reply operation missing failure kind".to_string()),
        ))
    }

    pub(in super::super) async fn persist_post_provider_operation(
        &self,
        operation: CoachConversationReplyOperation,
        write_label: &'static str,
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let repo = self.reply_operations.clone();
        let ctx = RetryContext {
            write_label,
            user_message_id: operation.user_message_id.clone(),
            scope_label: "conversation_id",
            scope_value: operation.scope_id.clone(),
            operation_status: Some(format!("{:?}", operation.status)),
        };

        retry_persist(
            RetryConfig {
                max_attempts: POST_PROVIDER_WRITE_ATTEMPTS,
                backoff_base_ms: 25,
            },
            |e| matches!(e, CoachConversationError::Repository(_)),
            || {
                let op = operation.clone();
                let r = repo.clone();
                Box::pin(async move { r.upsert(op).await })
            },
            &ctx,
        )
        .await
    }

    async fn replace_provider_transcript(
        &self,
        conversation: &CoachConversation,
        provider_transcript: Vec<LlmChatMessage>,
    ) -> Result<(), CoachConversationError> {
        let updated_at_epoch_seconds = next_provider_transcript_updated_at_epoch_seconds(
            conversation.updated_at_epoch_seconds,
            self.clock.now_epoch_seconds(),
        );
        self.conversations
            .replace_provider_transcript(
                &conversation.user_id,
                &conversation.conversation_id,
                provider_transcript,
                conversation.updated_at_epoch_seconds,
                updated_at_epoch_seconds,
            )
            .await
    }
}
