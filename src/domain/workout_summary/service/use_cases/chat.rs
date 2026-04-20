use tracing::{info, warn};

use super::*;

enum CoachReplyOperationResolution {
    Continue(CoachReplyOperation),
    Reply(CoachReply),
    Error(WorkoutSummaryError),
}

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
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let user_message = self
            .append_message_with_role(user_id, workout_id, MessageRole::User, content)
            .await?;

        let summary = self.get_existing_summary(user_id, workout_id).await?;
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

        Ok(PersistedUserMessage {
            summary,
            user_message,
            athlete_summary_may_regenerate_before_reply,
        })
    }

    pub(super) async fn generate_coach_reply_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let user_message = self
            .load_persisted_user_message(user_id, workout_id, &user_message_id)
            .await?;
        let operation = match self
            .claim_coach_reply_operation(user_id, workout_id, &user_message)
            .await?
        {
            CoachReplyOperationResolution::Continue(operation) => operation,
            CoachReplyOperationResolution::Reply(reply) => return Ok(reply),
            CoachReplyOperationResolution::Error(error) => return Err(error),
        };

        info!(
            workout_id = %workout_id,
            user_message_id = %user_message.id,
            attempt_count = operation.attempt_count,
            "requesting workout summary coach reply"
        );

        let (operation, llm_response, athlete_summary_was_regenerated) = self
            .request_and_checkpoint_coach_reply(user_id, workout_id, &user_message, operation)
            .await?;
        let coach_message = self
            .append_coach_reply_message(user_id, workout_id, &operation, &llm_response)
            .await?;
        self.finalize_coach_reply_operation(operation, &llm_response, &coach_message)
            .await?;

        self.build_coach_reply_result(
            user_id,
            workout_id,
            coach_message,
            athlete_summary_was_regenerated,
        )
        .await
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

    async fn claim_coach_reply_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
    ) -> Result<CoachReplyOperationResolution, WorkoutSummaryError> {
        let pending_operation =
            self.build_pending_coach_reply_operation(user_id, workout_id, user_message);
        let stale_before_epoch_seconds =
            self.clock.now_epoch_seconds() - Self::STALE_PENDING_TIMEOUT_SECONDS;

        match self
            .reply_operations
            .claim_pending(pending_operation, stale_before_epoch_seconds)
            .await?
        {
            CoachReplyClaimResult::Claimed(operation) => {
                self.handle_claimed_coach_reply_operation(
                    user_id,
                    workout_id,
                    user_message,
                    operation,
                )
                .await
            }
            CoachReplyClaimResult::Existing(existing) => {
                self.handle_existing_coach_reply_operation(
                    user_id,
                    workout_id,
                    user_message,
                    existing,
                )
                .await
            }
        }
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

    async fn handle_claimed_coach_reply_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
        operation: CoachReplyOperation,
    ) -> Result<CoachReplyOperationResolution, WorkoutSummaryError> {
        if let Some(reply) = self
            .try_recover_pending_operation(user_id, workout_id, &user_message.id, &operation)
            .await?
        {
            return Ok(CoachReplyOperationResolution::Reply(reply));
        }

        Ok(CoachReplyOperationResolution::Continue(operation))
    }

    async fn handle_existing_coach_reply_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
        existing: CoachReplyOperation,
    ) -> Result<CoachReplyOperationResolution, WorkoutSummaryError> {
        match existing.status {
            CoachReplyOperationStatus::Completed => Ok(CoachReplyOperationResolution::Reply(
                self.get_completed_reply(user_id, workout_id, existing)
                    .await?,
            )),
            CoachReplyOperationStatus::Failed => Ok(CoachReplyOperationResolution::Error(
                self.map_existing_llm_failure(existing),
            )),
            CoachReplyOperationStatus::Pending => {
                if let Some(reply) = self
                    .try_recover_pending_operation(user_id, workout_id, &user_message.id, &existing)
                    .await?
                {
                    return Ok(CoachReplyOperationResolution::Reply(reply));
                }

                Ok(CoachReplyOperationResolution::Error(
                    WorkoutSummaryError::ReplyAlreadyPending,
                ))
            }
        }
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
            crate::domain::llm::LlmChatResponse,
            bool,
        ),
        WorkoutSummaryError,
    > {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        let (athlete_summary_text, athlete_summary_was_regenerated) =
            self.ensure_athlete_summary(user_id).await?;

        let llm_response = self
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
            .persist_provider_response_checkpoint(operation, &llm_response)
            .await?;

        Ok((operation, llm_response, athlete_summary_was_regenerated))
    }

    async fn request_coach_reply_from_llm(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message: &ConversationMessage,
        summary: &WorkoutSummary,
        athlete_summary_text: Option<&str>,
        operation: &CoachReplyOperation,
    ) -> Result<crate::domain::llm::LlmChatResponse, WorkoutSummaryError> {
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
        operation: CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let operation = operation.record_provider_response(PendingCoachReplyCheckpoint {
            provider: llm_response.provider.clone(),
            model: llm_response.model.clone(),
            provider_request_id: llm_response.provider_request_id.clone(),
            provider_cache_id: llm_response.cache.provider_cache_id.clone(),
            token_usage: llm_response.usage.clone(),
            cache_usage: llm_response.cache.clone(),
            response_message: llm_response.message.clone(),
            updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
        });

        self.persist_post_provider_operation(operation, "persist_success_checkpoint")
            .await
    }

    async fn append_coach_reply_message(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "pending coach reply operation missing reserved coach message id".to_string(),
            )
        })?;

        self.append_message_with_role_and_id(
            user_id,
            workout_id,
            MessageRole::Coach,
            llm_response.message.clone(),
            Some(coach_message_id),
            false,
        )
        .await
    }

    async fn finalize_coach_reply_operation(
        &self,
        operation: CoachReplyOperation,
        llm_response: &crate::domain::llm::LlmChatResponse,
        coach_message: &ConversationMessage,
    ) -> Result<(), WorkoutSummaryError> {
        let completed = operation.mark_completed(CompletedCoachReply {
            provider: llm_response.provider.clone(),
            model: llm_response.model.clone(),
            provider_request_id: llm_response.provider_request_id.clone(),
            coach_message_id: coach_message.id.clone(),
            provider_cache_id: llm_response.cache.provider_cache_id.clone(),
            token_usage: llm_response.usage.clone(),
            cache_usage: llm_response.cache.clone(),
            updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
        });
        self.persist_post_provider_operation(completed, "persist_completed_reply")
            .await?;
        Ok(())
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
