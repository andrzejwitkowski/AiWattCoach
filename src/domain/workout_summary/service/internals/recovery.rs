use std::time::Duration;

use crate::domain::{
    llm::{last_nonempty_assistant_content, merge_provider_transcript_entries},
    llm_tools::public_tool_call_from_llm,
};

use tracing::{info, warn};

use super::super::*;
use super::AppendMessageInput;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(in super::super) async fn merge_provider_transcript_with_retry(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        write_label: &'static str,
    ) -> Result<(), WorkoutSummaryError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            let summary = self.get_existing_summary(user_id, workout_id).await?;
            let merged = merge_provider_transcript_entries(
                summary.provider_transcript,
                &operation.provider_transcript,
            );

            match self
                .replace_provider_transcript(
                    user_id,
                    workout_id,
                    summary.updated_at_epoch_seconds,
                    merged,
                )
                .await
            {
                Ok(()) => {
                    if attempt > 1 {
                        info!(
                            workout_id = %workout_id,
                            user_message_id = %operation.user_message_id,
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            write_label,
                            "recovered provider transcript write after retry"
                        );
                    }
                    return Ok(());
                }
                Err(error @ WorkoutSummaryError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        workout_id = %workout_id,
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
            WorkoutSummaryError::Repository(
                "provider transcript write failed without error".to_string(),
            )
        }))
    }

    pub(in super::super) async fn get_completed_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let coach_message_id = operation.coach_message_id.ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "completed coach reply operation missing coach message id".to_string(),
            )
        })?;
        let coach_message = self
            .get_message_by_id(user_id, workout_id, &coach_message_id)
            .await?;
        let summary = self.get_existing_summary(user_id, workout_id).await?;

        Ok(CoachReply {
            summary,
            coach_message,
            athlete_summary_was_regenerated: false,
        })
    }

    pub(in super::super) fn map_existing_llm_failure(
        &self,
        operation: CoachReplyOperation,
    ) -> WorkoutSummaryError {
        if let Some(failure_kind) = operation.failure_kind {
            return WorkoutSummaryError::Llm(failure_kind.to_llm_error(operation.error_message));
        }

        WorkoutSummaryError::Llm(crate::domain::llm::LlmError::Internal(
            operation
                .error_message
                .unwrap_or_else(|| "failed coach reply operation missing failure kind".to_string()),
        ))
    }

    pub(in super::super) async fn try_recover_pending_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        if let Some(reply) = self
            .recover_from_existing_coach_message(user_id, workout_id, user_message_id, operation)
            .await?
        {
            return Ok(Some(reply));
        }

        if operation.provider_transcript.is_empty() {
            return Ok(None);
        }

        self.recover_from_provider_transcript(user_id, workout_id, user_message_id, operation)
            .await
    }

    pub(in super::super) async fn persist_post_provider_operation(
        &self,
        operation: CoachReplyOperation,
        write_label: &'static str,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            match self.reply_operations.upsert(operation.clone()).await {
                Ok(saved) => {
                    if attempt > 1 {
                        info!(
                            workout_id = %saved.workout_id,
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
                Err(error @ WorkoutSummaryError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        workout_id = %operation.workout_id,
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
            WorkoutSummaryError::Repository(
                "post-provider coach reply write failed without error".to_string(),
            )
        }))
    }

    pub(in super::super) async fn ensure_athlete_summary(
        &self,
        user_id: &str,
    ) -> Result<(Option<String>, bool), WorkoutSummaryError> {
        let Some(service) = &self.athlete_summary_service else {
            return Ok((None, false));
        };

        let ensured = match service.ensure_fresh_summary_state(user_id).await {
            Ok(ensured) => ensured,
            Err(crate::domain::athlete_summary::AthleteSummaryError::Llm(error)) => {
                return Err(WorkoutSummaryError::Llm(error));
            }
            Err(error) => {
                warn!(
                    user_id = %user_id,
                    error = %error,
                    "athlete summary skipped while generating coach reply"
                );
                return Ok((None, false));
            }
        };

        Ok((Some(ensured.summary.summary_text), ensured.was_regenerated))
    }

    async fn recover_from_existing_coach_message(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        let Some(existing_coach_message_id) = operation.coach_message_id.clone() else {
            return Ok(None);
        };

        let Some(existing_coach_message) = self
            .repository
            .find_message_by_id(user_id, workout_id, &existing_coach_message_id)
            .await?
        else {
            return Ok(None);
        };

        let completed = operation.mark_completed_from_existing_message(
            existing_coach_message.id.clone(),
            self.clock.now_epoch_seconds(),
        );
        self.persist_post_provider_operation(completed, "recover_existing_coach_message")
            .await?;
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        info!(
            workout_id = %workout_id,
            user_message_id = %user_message_id,
            coach_message_id = %existing_coach_message.id,
            "recovered coach reply from persisted message"
        );

        Ok(Some(CoachReply {
            summary,
            coach_message: existing_coach_message,
            athlete_summary_was_regenerated: false,
        }))
    }

    async fn recover_from_provider_transcript(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        if let Err(error) = self
            .merge_provider_transcript_with_retry(
                user_id,
                workout_id,
                operation,
                "recover_provider_transcript",
            )
            .await
        {
            return self
                .fail_provider_transcript_recovery(
                    operation,
                    format!("failed to persist provider transcript during recovery: {error}"),
                )
                .await;
        }

        self.materialize_missing_recovery_tool_messages(user_id, workout_id, operation)
            .await?;

        if let Some(content) = recoverable_assistant_content(operation) {
            let coach_message = self
                .append_recovered_coach_message(user_id, workout_id, operation, content)
                .await?;
            let completed = operation.mark_completed_from_existing_message(
                coach_message.id.clone(),
                self.clock.now_epoch_seconds(),
            );
            self.persist_post_provider_operation(completed, "replay_persisted_coach_reply")
                .await?;
            let summary = self.get_existing_summary(user_id, workout_id).await?;
            info!(
                workout_id = %workout_id,
                user_message_id = %user_message_id,
                coach_message_id = %coach_message.id,
                "replayed persisted coach reply after partial crash"
            );
            return Ok(Some(CoachReply {
                summary,
                coach_message,
                athlete_summary_was_regenerated: false,
            }));
        }

        self.fail_invalid_provider_transcript_recovery(operation)
            .await
    }

    async fn fail_provider_transcript_recovery(
        &self,
        operation: &CoachReplyOperation,
        message: String,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        let llm_error = crate::domain::llm::LlmError::Internal(message);
        let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
        self.persist_post_provider_operation(failed, "persist_failed_provider_transcript_recovery")
            .await?;
        Err(WorkoutSummaryError::Llm(llm_error))
    }

    async fn materialize_missing_recovery_tool_messages(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<(), WorkoutSummaryError> {
        for transcript_message in &operation.provider_transcript {
            if transcript_message.role != crate::domain::llm::LlmMessageRole::Assistant {
                continue;
            }

            for tool_call in &transcript_message.tool_calls {
                if self
                    .recovery_tool_call_already_materialized(
                        user_id,
                        workout_id,
                        operation,
                        &tool_call.id,
                    )
                    .await?
                {
                    continue;
                }

                self.append_tool_message(user_id, workout_id, public_tool_call_from_llm(tool_call))
                    .await?;
            }
        }

        Ok(())
    }

    async fn recovery_tool_call_already_materialized(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        tool_call_id: &str,
    ) -> Result<bool, WorkoutSummaryError> {
        if operation
            .public_tool_call_ids
            .iter()
            .any(|id| id == tool_call_id)
        {
            return Ok(true);
        }

        match self
            .get_message_by_id(user_id, workout_id, tool_call_id)
            .await
        {
            Ok(_) => Ok(true),
            Err(WorkoutSummaryError::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn append_recovered_coach_message(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        content: String,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "pending coach reply operation missing reserved coach message id".to_string(),
            )
        })?;

        self.append_message_with_role_and_id(
            user_id,
            workout_id,
            AppendMessageInput::coach(content, coach_message_id),
        )
        .await
    }

    async fn fail_invalid_provider_transcript_recovery(
        &self,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        let error = crate::domain::llm::LlmError::InvalidResponse(
            "assistant reply missing final text message".to_string(),
        );
        let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
        self.persist_post_provider_operation(failed, "replay_invalid_coach_reply")
            .await?;
        Err(WorkoutSummaryError::Llm(error))
    }
}

fn recoverable_assistant_content(operation: &CoachReplyOperation) -> Option<String> {
    last_nonempty_assistant_content(&operation.provider_transcript)
}
