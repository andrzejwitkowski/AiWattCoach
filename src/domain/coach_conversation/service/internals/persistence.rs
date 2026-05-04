use std::time::Duration;

use crate::domain::coach_conversation::CoachConversationReplyOperation;
use crate::domain::llm::{
    merge_provider_transcript_entries, next_provider_transcript_updated_at_epoch_seconds,
    LlmChatMessage, LlmError,
};
use tracing::{info, warn};

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
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            let latest = self
                .conversations
                .find_by_user_id_and_conversation_id(
                    &conversation.user_id,
                    &conversation.conversation_id,
                )
                .await?
                .ok_or(CoachConversationError::NotFound)?;
            let merged = merge_provider_transcript_entries(
                latest.provider_transcript.clone(),
                &operation.provider_transcript,
            );

            match self.replace_provider_transcript(&latest, merged).await {
                Ok(()) => {
                    if attempt > 1 {
                        info!(
                            conversation_id = %conversation.conversation_id,
                            user_message_id = %operation.user_message_id,
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            write_label,
                            "recovered provider transcript write after retry"
                        );
                    }
                    return Ok(());
                }
                Err(error @ CoachConversationError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        conversation_id = %conversation.conversation_id,
                        user_message_id = %operation.user_message_id,
                        attempt,
                        max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                        write_label,
                        error = %error,
                        "retrying provider transcript write after repository error"
                    );
                    last_error = Some(error);
                    tokio::time::sleep(Duration::from_millis(25 * attempt as u64)).await;
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            CoachConversationError::Repository(
                "provider transcript write failed without error".to_string(),
            )
        }))
    }

    pub(in super::super) fn map_existing_llm_failure(
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
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            match self.reply_operations.upsert(operation.clone()).await {
                Ok(saved) => {
                    if attempt > 1 {
                        info!(
                            conversation_id = %saved.conversation_id,
                            user_message_id = %saved.user_message_id,
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            operation_status = ?saved.status,
                            write_label,
                            "recovered post-provider coach reply write after retry"
                        );
                    }
                    return Ok(saved);
                }
                Err(error @ CoachConversationError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        conversation_id = %operation.conversation_id,
                        user_message_id = %operation.user_message_id,
                        attempt,
                        max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                        operation_status = ?operation.status,
                        write_label,
                        error = %error,
                        "retrying post-provider coach reply write after repository error"
                    );
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            CoachConversationError::Repository(
                "post-provider coach reply write failed without error".to_string(),
            )
        }))
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
