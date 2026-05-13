use crate::domain::{
    coach_conversation::{CoachConversationMessageRole, CoachConversationReplyOperation},
    llm::{last_nonempty_assistant_content, LlmError, LlmMessageRole},
    llm_tools::public_tool_call_from_llm,
    public_tool_calls::materialization::materialize_public_tool_calls_idempotently,
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
        let Some(existing_coach_message_id) = operation.reply_message_id.clone() else {
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

        let operation = self
            .materialize_recovered_tool_messages(conversation, operation)
            .await?;

        let Some(content) = recovered_assistant_reply_text(&operation) else {
            let error =
                LlmError::InvalidResponse("assistant reply missing final text message".to_string());
            let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(failed, "replay_invalid_conversation_reply")
                .await?;
            return Err(CoachConversationError::Llm(error));
        };
        let reasoning_content = recovered_reasoning_content(&operation);

        let coach_message_id = operation.reply_message_id.clone().ok_or_else(|| {
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
                reasoning_content,
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
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let mut operation = operation.clone();
        let public_tool_calls: Vec<_> = operation
            .provider_transcript
            .iter()
            .filter(|message| message.role == LlmMessageRole::Assistant)
            .flat_map(|message| message.tool_calls.iter())
            .map(public_tool_call_from_llm)
            .collect();
        let conversation = conversation.clone();
        let service = self.clone();

        operation.public_tool_call_ids = materialize_public_tool_calls_idempotently(
            operation.public_tool_call_ids.clone(),
            &public_tool_calls,
            |tool_call_id| {
                let service = service.clone();
                let conversation = conversation.clone();
                let tool_call_id = tool_call_id.to_string();
                async move {
                    service
                        .tool_message_already_materialized(&conversation, &tool_call_id)
                        .await
                }
            },
            |tool_call| {
                let service = service.clone();
                let conversation = conversation.clone();
                async move {
                    service
                        .append_tool_message(&conversation, tool_call)
                        .await
                        .map(|_| ())
                }
            },
        )
        .await?;

        Ok(operation)
    }
}

fn recovered_assistant_reply_text(operation: &CoachConversationReplyOperation) -> Option<String> {
    last_nonempty_assistant_content(&operation.provider_transcript)
}

fn recovered_reasoning_content(operation: &CoachConversationReplyOperation) -> Option<String> {
    operation
        .provider_transcript
        .iter()
        .rev()
        .find(|m| m.role == LlmMessageRole::Assistant)
        .and_then(|m| m.reasoning_content.clone())
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        coach_conversation::{CoachConversationError, CoachConversationReplyOperation},
        llm::{
            LlmCacheUsage, LlmChatMessage, LlmProvider, LlmTokenUsage, LlmToolCall,
            PendingLlmReplyCheckpoint,
        },
        public_tool_calls::materialization::materialize_public_tool_calls_idempotently,
        workout_summary::PublicToolCall,
    };

    use super::*;

    #[tokio::test]
    async fn recovered_tool_materialization_stays_idempotent_for_same_transcript() {
        let operation = CoachConversationReplyOperation::pending(
            "user-1".to_string(),
            "conversation-1".to_string(),
            "message-1".to_string(),
            Some("calendar-coach:user-1:conversation-1".to_string()),
            "coach-message-1".to_string(),
            1_700_000_000,
        )
        .record_provider_response(PendingLlmReplyCheckpoint {
            provider: LlmProvider::OpenAi,
            model: "gpt-4o-mini".to_string(),
            provider_request_id: None,
            provider_cache_id: None,
            token_usage: LlmTokenUsage::default(),
            cache_usage: LlmCacheUsage::default(),
            provider_transcript: vec![LlmChatMessage::assistant_with_tool_calls(
                "",
                vec![
                    LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "first".to_string(),
                        arguments_json: "{}".to_string(),
                    },
                    LlmToolCall {
                        id: "tool-2".to_string(),
                        name: "second".to_string(),
                        arguments_json: "{}".to_string(),
                    },
                ],
            )],
            finish_reason: None,
            updated_at_epoch_seconds: 1_700_000_001,
        });

        let public_tool_calls: Vec<PublicToolCall> = operation
            .provider_transcript
            .iter()
            .filter(|message| message.role == LlmMessageRole::Assistant)
            .flat_map(|message| message.tool_calls.iter())
            .map(public_tool_call_from_llm)
            .collect();

        let appended = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let materialized_once = materialize_public_tool_calls_idempotently(
            operation.public_tool_call_ids.clone(),
            &public_tool_calls,
            |_| async { Ok::<bool, CoachConversationError>(false) },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), CoachConversationError>(())
                    }
                }
            },
        )
        .await
        .expect("first recovery materialization should succeed");

        let materialized_twice = materialize_public_tool_calls_idempotently(
            materialized_once,
            &public_tool_calls,
            |_| async { Ok::<bool, CoachConversationError>(false) },
            {
                let appended = appended.clone();
                move |tool_call| {
                    let appended = appended.clone();
                    async move {
                        appended.lock().await.push(tool_call.id);
                        Ok::<(), CoachConversationError>(())
                    }
                }
            },
        )
        .await
        .expect("second recovery materialization should stay idempotent");

        assert_eq!(
            materialized_twice,
            vec!["tool-1".to_string(), "tool-2".to_string()]
        );
        assert_eq!(
            appended.lock().await.clone(),
            vec!["tool-1".to_string(), "tool-2".to_string()]
        );
    }
}
