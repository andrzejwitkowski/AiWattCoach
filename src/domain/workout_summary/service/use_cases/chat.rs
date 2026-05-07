use tracing::{info, warn};

use crate::domain::llm::{
    final_assistant_text, resolve_llm_reply_operation, LlmReplyClaimResult, LlmReplyOperation,
    LlmReplyResolutionWorkflow, ResolvedLlmReplyOperation,
};

use super::*;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(super) async fn send_message_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> Result<SendMessageResult, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let persisted = self
            .append_user_message_impl(user_id, workout_id, content)
            .await?;
        let reply = self
            .generate_coach_reply_impl(user_id, workout_id, persisted.user_message.id.clone())
            .await?;

        Ok(SendMessageResult {
            summary: reply.summary,
            user_message: persisted.user_message,
            coach_message: reply.coach_message,
        })
    }

    pub(super) async fn append_user_message_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> Result<PersistedUserMessage, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;

        let user_message = self
            .append_message_with_role(
                user_id,
                &target.storage_workout_id,
                MessageRole::User,
                content,
            )
            .await?;

        let summary = self
            .get_existing_summary(user_id, &target.storage_workout_id)
            .await?;
        let athlete_summary_may_regenerate_before_reply =
            if let Some(athlete_summary_service) = &self.athlete_summary_service {
                match athlete_summary_service.get_summary_state(user_id).await {
                    Ok(state) => state.stale,
                    Err(error) => {
                        warn!(
                            user_id = %user_id,
                            workout_id = %workout_id,
                            error = %error,
                            "athlete summary hint lookup failed while appending user message"
                        );
                        false
                    }
                }
            } else {
                false
            };

        Ok(self.present_persisted_user_message(
            PersistedUserMessage {
                summary,
                user_message,
                athlete_summary_may_regenerate_before_reply,
            },
            &target.requested_workout_id,
        ))
    }

    pub(super) async fn generate_coach_reply_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;

        let user_message = self
            .load_persisted_user_message(user_id, &target.storage_workout_id, &user_message_id)
            .await?;
        let pending_operation = self.build_pending_coach_reply_operation(
            user_id,
            &target.storage_workout_id,
            &user_message,
        );
        let operation = match resolve_llm_reply_operation(self, pending_operation).await? {
            ResolvedLlmReplyOperation::Continue(operation) => *operation,
            ResolvedLlmReplyOperation::Reply(reply) => {
                return Ok(self.present_coach_reply(reply, &target.requested_workout_id));
            }
            ResolvedLlmReplyOperation::Error(error) => return Err(error),
        };

        info!(
            workout_id = %target.storage_workout_id,
            user_message_id = %user_message.id,
            attempt_count = operation.attempt_count,
            "requesting workout summary coach reply"
        );

        let (operation, llm_output, athlete_summary_was_regenerated) = self
            .request_and_checkpoint_coach_reply(
                user_id,
                &target.storage_workout_id,
                &user_message,
                operation,
            )
            .await?;
        let coach_message = self
            .append_coach_reply_message(
                user_id,
                &target.storage_workout_id,
                &operation,
                &llm_output.response,
            )
            .await?;
        self.finalize_coach_reply_operation(operation, &llm_output.response, &coach_message)
            .await?;

        self.build_coach_reply_result(
            user_id,
            &target.storage_workout_id,
            coach_message,
            athlete_summary_was_regenerated,
        )
        .await
        .map(|reply| self.present_coach_reply(reply, &target.requested_workout_id))
    }

    async fn load_persisted_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let user_message = self
            .get_message_by_id(user_id, workout_id, user_message_id)
            .await?;

        if user_message.role != MessageRole::User {
            return Err(WorkoutSummaryError::Validation(
                "user message must be persisted before generating coach reply".to_string(),
            ));
        }

        Ok(user_message)
    }

    fn build_pending_coach_reply_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
    ) -> CoachReplyOperation {
        let now = self.clock.now_epoch_seconds();
        CoachReplyOperation::pending(
            user_id.to_string(),
            workout_id.to_string(),
            user_message.id.clone(),
            Some(format!("workout-summary:{user_id}:{workout_id}")),
            self.ids.new_id("message"),
            now,
        )
    }

    async fn request_and_checkpoint_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
        operation: CoachReplyOperation,
    ) -> Result<
        (
            CoachReplyOperation,
            crate::domain::llm_tools::LlmToolLoopOutput,
            bool,
        ),
        WorkoutSummaryError,
    > {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        let (athlete_summary_text, athlete_summary_was_regenerated) =
            self.ensure_athlete_summary(user_id).await?;

        let llm_output = self
            .request_coach_reply_from_llm(
                user_id,
                workout_id,
                user_message,
                &summary,
                athlete_summary_text.as_deref(),
                &operation,
            )
            .await?;
        let operation = self
            .persist_provider_response_checkpoint(user_id, workout_id, operation, &llm_output)
            .await?;
        let operation = self
            .materialize_public_tool_messages(
                user_id,
                workout_id,
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

        Ok((operation, llm_output, athlete_summary_was_regenerated))
    }

    async fn request_coach_reply_from_llm(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
        summary: &WorkoutSummary,
        athlete_summary_text: Option<&str>,
        operation: &CoachReplyOperation,
    ) -> Result<crate::domain::llm_tools::LlmToolLoopOutput, WorkoutSummaryError> {
        match self
            .coach
            .reply(
                user_id,
                summary,
                &user_message.content,
                athlete_summary_text,
            )
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
                self.persist_post_provider_operation(failed, "persist_failed_checkpoint")
                    .await?;
                warn!(
                    user_id = %user_id,
                    workout_id = %workout_id,
                    user_message_id = %user_message.id,
                    retryable = error.is_retryable(),
                    error = %error,
                    "workout summary coach reply failed"
                );
                Err(WorkoutSummaryError::Llm(error))
            }
        }
    }

    async fn persist_provider_response_checkpoint(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
        llm_output: &crate::domain::llm_tools::LlmToolLoopOutput,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let operation =
            operation.record_provider_response(self.build_pending_reply_checkpoint(llm_output));
        let operation = self
            .persist_post_provider_operation(operation, "persist_success_checkpoint")
            .await?;
        if let Err(error) = self
            .merge_provider_transcript_with_retry(
                user_id,
                workout_id,
                &operation,
                "persist_provider_transcript",
            )
            .await
        {
            let llm_error = crate::domain::llm::LlmError::Internal(format!(
                "failed to persist provider transcript after provider response: {error}"
            ));
            let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(
                failed,
                "persist_failed_provider_transcript_checkpoint",
            )
            .await?;
            return Err(WorkoutSummaryError::Llm(llm_error));
        }

        Ok(operation)
    }

    async fn require_final_assistant_text(
        &self,
        operation: &CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
    ) -> Result<String, WorkoutSummaryError> {
        let Some(coach_content) = final_assistant_text(llm_response) else {
            let error = crate::domain::llm::LlmError::InvalidResponse(
                "assistant reply missing final text message".to_string(),
            );
            let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(failed, "persist_invalid_response_checkpoint")
                .await?;
            return Err(WorkoutSummaryError::Llm(error));
        };

        Ok(coach_content)
    }

    async fn append_coach_reply_message(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let coach_content = self
            .require_final_assistant_text(operation, llm_response)
            .await?;
        let coach_message_id = operation.reply_message_id.clone().ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "pending coach reply operation missing reserved coach message id".to_string(),
            )
        })?;

        self.append_message_with_role_and_id(
            user_id,
            workout_id,
            crate::domain::workout_summary::service::internals::AppendMessageInput::coach(
                coach_content,
                coach_message_id,
            ),
        )
        .await
    }

    async fn finalize_coach_reply_operation(
        &self,
        operation: CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
        coach_message: &ConversationMessage,
    ) -> Result<(), WorkoutSummaryError> {
        let completed = operation
            .mark_completed(self.build_completed_reply(llm_response, coach_message.id.clone()));
        self.persist_post_provider_operation(completed, "persist_completed_reply")
            .await?;
        Ok(())
    }

    fn build_pending_reply_checkpoint(
        &self,
        llm_output: &crate::domain::llm_tools::LlmToolLoopOutput,
    ) -> PendingCoachReplyCheckpoint {
        LlmReplyOperation::pending_checkpoint_from_tool_loop(
            llm_output,
            self.clock.now_epoch_seconds(),
        )
    }

    fn build_completed_reply(
        &self,
        llm_response: &crate::domain::llm::LlmChatResponse,
        reply_message_id: String,
    ) -> CompletedCoachReply {
        LlmReplyOperation::completed_reply_from_response(
            llm_response,
            reply_message_id,
            self.clock.now_epoch_seconds(),
        )
    }

    async fn build_coach_reply_result(
        &self,
        user_id: &str,
        workout_id: &str,
        coach_message: ConversationMessage,
        athlete_summary_was_regenerated: bool,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        Ok(CoachReply {
            summary,
            coach_message,
            athlete_summary_was_regenerated,
        })
    }
}

impl<Repo, Ops, Time, Ids> LlmReplyResolutionWorkflow
    for WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    type Reply = CoachReply;
    type Error = WorkoutSummaryError;

    fn stale_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - STALE_PENDING_TIMEOUT_SECONDS
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> crate::domain::llm::BoxFuture<Result<LlmReplyClaimResult, Self::Error>> {
        let reply_operations = self.reply_operations.clone();
        Box::pin(async move {
            reply_operations
                .claim_pending(operation, stale_before_epoch_seconds)
                .await
        })
    }

    fn recover_pending_operation(
        &self,
        operation: &CoachReplyOperation,
    ) -> crate::domain::llm::BoxFuture<Result<Option<Self::Reply>, Self::Error>> {
        let service = self.clone();
        let operation = operation.clone();
        Box::pin(async move {
            service
                .try_recover_pending_operation(
                    &operation.user_id,
                    &operation.scope_id,
                    &operation.user_message_id,
                    &operation,
                )
                .await
        })
    }

    fn get_completed_reply(
        &self,
        operation: CoachReplyOperation,
    ) -> crate::domain::llm::BoxFuture<Result<Self::Reply, Self::Error>> {
        let service = self.clone();
        let user_id = operation.user_id.clone();
        let scope_id = operation.scope_id.clone();
        Box::pin(async move {
            service
                .get_completed_reply(&user_id, &scope_id, operation)
                .await
        })
    }

    fn map_existing_llm_failure(&self, operation: CoachReplyOperation) -> Self::Error {
        self.map_existing_llm_failure(operation)
    }

    fn reply_already_pending_error(&self) -> Self::Error {
        WorkoutSummaryError::ReplyAlreadyPending
    }
}
