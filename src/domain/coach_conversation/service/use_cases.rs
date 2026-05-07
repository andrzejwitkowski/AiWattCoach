use crate::domain::llm::LlmError;
use crate::domain::llm::{
    resolve_llm_reply_operation, LlmReplyClaimResult, LlmReplyOperation,
    LlmReplyResolutionWorkflow, ResolvedLlmReplyOperation,
};

use super::{
    super::{
        BoxFuture, CoachConversation, CoachConversationError, CoachConversationMessage,
        CoachConversationMessageRole, CoachConversationReply, CoachConversationReplyOperation,
        CompletedCoachConversationReply, PendingCoachConversationReplyCheckpoint,
        PersistedConversationUserMessage, SendConversationMessageResult,
    },
    transcript::final_assistant_text,
    CoachConversationUseCases, SharedCoachConversationService, STALE_PENDING_TIMEOUT_SECONDS,
};

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    async fn request_and_checkpoint_calendar_reply(
        &self,
        conversation: &CoachConversation,
        messages: &[CoachConversationMessage],
        user_message: &CoachConversationMessage,
        operation: CoachConversationReplyOperation,
    ) -> Result<
        (
            CoachConversationReplyOperation,
            crate::domain::llm_tools::LlmToolLoopOutput,
        ),
        CoachConversationError,
    > {
        let llm_output = match self
            .request_reply_from_llm(conversation, messages, user_message)
            .await
        {
            Ok(response) => response,
            Err(CoachConversationError::Llm(error)) => {
                let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
                self.persist_post_provider_operation(failed, "persist_failed_checkpoint")
                    .await?;
                tracing::warn!(
                    user_id = %conversation.user_id,
                    conversation_id = %conversation.conversation_id,
                    user_message_id = %user_message.id,
                    retryable = error.is_retryable(),
                    error = %error,
                    "coach conversation reply failed"
                );
                return Err(CoachConversationError::Llm(error));
            }
            Err(error) => return Err(error),
        };
        let operation = self
            .persist_provider_response_checkpoint(conversation, operation, &llm_output)
            .await?;
        let operation = self
            .materialize_public_tool_messages(
                conversation,
                operation,
                &llm_output.state.public_tool_calls,
            )
            .await?;
        let operation = if llm_output.state.public_tool_calls.is_empty() {
            operation
        } else {
            self.persist_post_provider_operation(operation, "persist_public_tool_messages")
                .await?
        };

        Ok((operation, llm_output))
    }

    async fn persist_provider_response_checkpoint(
        &self,
        conversation: &CoachConversation,
        operation: CoachConversationReplyOperation,
        llm_output: &crate::domain::llm_tools::LlmToolLoopOutput,
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let operation =
            operation.record_provider_response(self.build_pending_reply_checkpoint(llm_output));
        let operation = self
            .persist_post_provider_operation(operation, "persist_provider_response_checkpoint")
            .await?;

        if let Err(error) = self
            .merge_provider_transcript_with_retry(
                conversation,
                &operation,
                "persist_provider_transcript",
            )
            .await
        {
            let llm_error = LlmError::Internal(format!(
                "failed to persist provider transcript after provider response: {error}"
            ));
            let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(
                failed,
                "persist_failed_provider_transcript_checkpoint",
            )
            .await?;
            return Err(CoachConversationError::Llm(llm_error));
        }

        Ok(operation)
    }

    fn build_pending_reply_checkpoint(
        &self,
        llm_output: &crate::domain::llm_tools::LlmToolLoopOutput,
    ) -> PendingCoachConversationReplyCheckpoint {
        LlmReplyOperation::pending_checkpoint_from_tool_loop(
            llm_output,
            self.clock.now_epoch_seconds(),
        )
    }

    fn build_completed_reply(
        &self,
        llm_response: &crate::domain::llm::LlmChatResponse,
        reply_message_id: String,
    ) -> CompletedCoachConversationReply {
        LlmReplyOperation::completed_reply_from_response(
            llm_response,
            reply_message_id,
            self.clock.now_epoch_seconds(),
        )
    }
}

impl<Conversations, Messages, Ops, Time, Ids> CoachConversationUseCases
    for SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    fn get_or_create_active_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let conversation = if let Some(existing) = service
                .conversations
                .find_active_by_user_id_and_surface(
                    &user_id,
                    &super::super::CoachConversationSurface::Calendar,
                )
                .await?
            {
                existing
            } else {
                service.create_calendar_conversation(&user_id).await?
            };
            let messages = service
                .list_messages(&user_id, &conversation.conversation_id)
                .await?;
            Ok((conversation, messages))
        })
    }

    fn start_new_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service
                .archive_active_calendar_conversation_if_present(&user_id)
                .await?;
            let conversation = service.create_calendar_conversation(&user_id).await?;
            Ok((conversation, Vec::new()))
        })
    }

    fn get_calendar_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_conversation(&user_id, &conversation_id)
                .await?;
            let messages = service.list_messages(&user_id, &conversation_id).await?;
            Ok((conversation, messages))
        })
    }

    fn send_calendar_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let persisted = service
                .append_calendar_user_message(&user_id, &conversation_id, content)
                .await?;
            let reply = service
                .generate_calendar_reply(
                    &user_id,
                    &conversation_id,
                    persisted.user_message.id.clone(),
                )
                .await?;
            Ok(SendConversationMessageResult {
                conversation: reply.conversation,
                messages: reply.messages,
                user_message: persisted.user_message,
                coach_message: reply.coach_message,
            })
        })
    }

    fn append_calendar_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&user_id, &conversation_id)
                .await?;
            let user_message = service
                .append_message(
                    &conversation,
                    CoachConversationMessageRole::User,
                    content,
                    None,
                    None,
                )
                .await?;
            let messages = service.list_messages(&user_id, &conversation_id).await?;

            Ok(PersistedConversationUserMessage {
                conversation,
                messages,
                user_message,
                athlete_summary_may_regenerate_before_reply: false,
            })
        })
    }

    fn generate_calendar_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&user_id, &conversation_id)
                .await?;
            let user_message = service
                .load_persisted_user_message(&user_id, &conversation_id, &user_message_id)
                .await?;
            let pending_operation =
                service.build_pending_reply_operation(&conversation, &user_message);
            let operation = match resolve_llm_reply_operation(&service, pending_operation).await? {
                ResolvedLlmReplyOperation::Continue(operation) => *operation,
                ResolvedLlmReplyOperation::Reply(reply) => return Ok(reply),
                ResolvedLlmReplyOperation::Error(error) => return Err(error),
            };

            let messages = service
                .list_messages(&conversation.user_id, &conversation.conversation_id)
                .await?;
            let (operation, llm_output) = service
                .request_and_checkpoint_calendar_reply(
                    &conversation,
                    &messages,
                    &user_message,
                    operation,
                )
                .await?;

            let Some(coach_content) = final_assistant_text(&llm_output.response) else {
                let error = LlmError::InvalidResponse(
                    "assistant reply missing final text message".to_string(),
                );
                let failed = operation.mark_failed(&error, service.clock.now_epoch_seconds());
                service
                    .persist_post_provider_operation(
                        failed,
                        "persist_invalid_conversation_response_checkpoint",
                    )
                    .await?;
                return Err(CoachConversationError::Llm(error));
            };

            let coach_message_id = operation.reply_message_id.clone().ok_or_else(|| {
                CoachConversationError::Repository(
                    "pending coach reply operation missing reserved coach message id".to_string(),
                )
            })?;
            let coach_message = service
                .append_message(
                    &conversation,
                    CoachConversationMessageRole::Coach,
                    coach_content,
                    Some(coach_message_id),
                    None,
                )
                .await?;
            let completed_reply =
                service.build_completed_reply(&llm_output.response, coach_message.id.clone());
            service
                .persist_post_provider_operation(
                    operation.mark_completed(completed_reply),
                    "finalize_completed_reply",
                )
                .await?;
            let messages = service
                .list_messages(&conversation.user_id, &conversation.conversation_id)
                .await?;

            Ok(CoachConversationReply {
                conversation,
                messages,
                coach_message,
                athlete_summary_was_regenerated: false,
            })
        })
    }
}

impl<Conversations, Messages, Ops, Time, Ids> LlmReplyResolutionWorkflow
    for SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    type Reply = CoachConversationReply;
    type Error = CoachConversationError;

    fn stale_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - STALE_PENDING_TIMEOUT_SECONDS
    }

    fn claim_pending(
        &self,
        operation: CoachConversationReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<LlmReplyClaimResult, Self::Error>> {
        let reply_operations = self.reply_operations.clone();
        Box::pin(async move {
            reply_operations
                .claim_pending(operation, stale_before_epoch_seconds)
                .await
        })
    }

    fn recover_pending_operation(
        &self,
        operation: &CoachConversationReplyOperation,
    ) -> BoxFuture<Result<Option<Self::Reply>, Self::Error>> {
        let service = self.clone();
        let operation = operation.clone();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&operation.user_id, &operation.scope_id)
                .await?;
            service
                .try_recover_pending_operation(&conversation, &operation)
                .await
        })
    }

    fn get_completed_reply(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> BoxFuture<Result<Self::Reply, Self::Error>> {
        let service = self.clone();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&operation.user_id, &operation.scope_id)
                .await?;
            service.get_completed_reply(&conversation, operation).await
        })
    }

    fn map_existing_llm_failure(&self, operation: CoachConversationReplyOperation) -> Self::Error {
        self.map_existing_llm_failure(operation)
    }

    fn reply_already_pending_error(&self) -> Self::Error {
        CoachConversationError::ReplyAlreadyPending
    }
}
