use crate::domain::{
    coach_conversation::{CoachConversationMessageRole, CoachConversationReplyOperation},
    llm::{last_nonempty_assistant_content, LlmError, LlmMessageRole},
    workout_summary::PublicToolCall,
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
    pub(in super::super) async fn try_recover_pending_operation(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<Option<CoachConversationReply>, CoachConversationError> {
        if let Some(reply) = self
            .recover_reply_from_existing_message(conversation, operation)
            .await?
        {
            return Ok(Some(reply));
        }

        if operation.provider_transcript.is_empty() {
            return Ok(None);
        }

        self.replay_persisted_reply_from_provider_transcript(conversation, operation)
            .await
    }

    async fn recover_reply_from_existing_message(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<Option<CoachConversationReply>, CoachConversationError> {
        let Some(existing_coach_message_id) = operation.coach_message_id.clone() else {
            return Ok(None);
        };

        let Some(existing_coach_message) = self
            .messages
            .find_by_user_id_and_conversation_id_and_message_id(
                &conversation.user_id,
                &conversation.conversation_id,
                &existing_coach_message_id,
            )
            .await?
        else {
            return Ok(None);
        };

        let completed = operation.mark_completed_from_existing_message(
            existing_coach_message.id.clone(),
            self.clock.now_epoch_seconds(),
        );
        self.persist_post_provider_operation(
            completed,
            "recover_existing_coach_conversation_message",
        )
        .await?;
        let messages = self
            .list_messages(&conversation.user_id, &conversation.conversation_id)
            .await?;

        Ok(Some(CoachConversationReply {
            conversation: conversation.clone(),
            messages,
            coach_message: existing_coach_message,
            athlete_summary_was_regenerated: false,
        }))
    }

    async fn replay_persisted_reply_from_provider_transcript(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<Option<CoachConversationReply>, CoachConversationError> {
        if let Err(error) = self
            .merge_provider_transcript_with_retry(
                conversation,
                operation,
                "recover_provider_transcript",
            )
            .await
        {
            let llm_error = LlmError::Internal(format!(
                "failed to persist provider transcript during recovery: {error}"
            ));
            let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(
                failed,
                "persist_failed_provider_transcript_recovery",
            )
            .await?;
            return Err(CoachConversationError::Llm(llm_error));
        }

        self.materialize_recovered_tool_messages(conversation, operation)
            .await?;

        let Some(content) = recovered_assistant_reply_text(operation) else {
            let error =
                LlmError::InvalidResponse("assistant reply missing final text message".to_string());
            let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(failed, "replay_invalid_conversation_reply")
                .await?;
            return Err(CoachConversationError::Llm(error));
        };

        let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
            CoachConversationError::Repository(
                "pending coach reply operation missing reserved coach message id".to_string(),
            )
        })?;
        let coach_message = self
            .append_message(
                conversation,
                CoachConversationMessageRole::Coach,
                content,
                Some(coach_message_id),
                None,
            )
            .await?;
        let completed = operation.mark_completed_from_existing_message(
            coach_message.id.clone(),
            self.clock.now_epoch_seconds(),
        );
        self.persist_post_provider_operation(completed, "replay_persisted_conversation_reply")
            .await?;
        let messages = self
            .list_messages(&conversation.user_id, &conversation.conversation_id)
            .await?;

        Ok(Some(CoachConversationReply {
            conversation: conversation.clone(),
            messages,
            coach_message,
            athlete_summary_was_regenerated: false,
        }))
    }

    async fn materialize_recovered_tool_messages(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<(), CoachConversationError> {
        for transcript_message in &operation.provider_transcript {
            if transcript_message.role != LlmMessageRole::Assistant {
                continue;
            }

            for tool_call in &transcript_message.tool_calls {
                if operation
                    .public_tool_call_ids
                    .iter()
                    .any(|id| id == &tool_call.id)
                    || self
                        .tool_message_already_materialized(conversation, &tool_call.id)
                        .await?
                {
                    continue;
                }

                self.append_tool_message(
                    conversation,
                    PublicToolCall {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments_json: tool_call.arguments_json.clone(),
                    },
                )
                .await?;
            }
        }

        Ok(())
    }
}

fn recovered_assistant_reply_text(operation: &CoachConversationReplyOperation) -> Option<String> {
    last_nonempty_assistant_content(&operation.provider_transcript)
}
