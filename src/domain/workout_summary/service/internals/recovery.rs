use crate::domain::{
    llm::{
        last_nonempty_assistant_content,
        persistence::{
            merge_provider_transcript_with_retry, retry_persist, ProviderTranscriptSnapshot,
            RetryConfig, RetryContext,
        },
    },
    llm_tools::public_tool_call_from_llm,
    public_tool_calls::materialization::materialize_public_tool_calls_idempotently,
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
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let provider_transcript = operation.provider_transcript.clone();
        let service = self.clone();
        let ctx = RetryContext {
            write_label,
            user_message_id: operation.user_message_id.clone(),
            scope_label: "workout_id",
            scope_value: workout_id.clone(),
            operation_status: None,
        };

        merge_provider_transcript_with_retry(
            RetryConfig {
                max_attempts: POST_PROVIDER_WRITE_ATTEMPTS,
                backoff_base_ms: 25,
            },
            |e| matches!(e, WorkoutSummaryError::Repository(_)),
            || {
                let svc = service.clone();
                let uid = user_id.clone();
                let wid = workout_id.clone();
                Box::pin(async move {
                    let summary = svc.get_existing_summary(&uid, &wid).await?;
                    Ok(ProviderTranscriptSnapshot {
                        latest_state: summary.updated_at_epoch_seconds,
                        provider_transcript: summary.provider_transcript,
                    })
                })
            },
            |expected_updated_at_epoch_seconds, merged| {
                let svc = service.clone();
                let uid = user_id.clone();
                let wid = workout_id.clone();
                Box::pin(async move {
                    svc.replace_provider_transcript(
                        &uid,
                        &wid,
                        expected_updated_at_epoch_seconds,
                        merged,
                    )
                    .await
                })
            },
            &provider_transcript,
            &ctx,
        )
        .await
    }

    pub(in super::super) async fn get_completed_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let coach_message_id = operation.reply_message_id.ok_or_else(|| {
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

    pub(in super::super) fn existing_llm_failure_to_error(
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
        let repo = self.reply_operations.clone();
        let ctx = RetryContext {
            write_label,
            user_message_id: operation.user_message_id.clone(),
            scope_label: "workout_id",
            scope_value: operation.scope_id.clone(),
            operation_status: Some(format!("{:?}", operation.status)),
        };

        retry_persist(
            RetryConfig {
                max_attempts: POST_PROVIDER_WRITE_ATTEMPTS,
                backoff_base_ms: 25,
            },
            |e| matches!(e, WorkoutSummaryError::Repository(_)),
            || {
                let op = operation.clone();
                let r = repo.clone();
                Box::pin(async move { r.upsert(op).await })
            },
            &ctx,
        )
        .await
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
        let Some(existing_coach_message_id) = operation.reply_message_id.clone() else {
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

        let operation = self
            .materialize_missing_recovery_tool_messages(user_id, workout_id, operation)
            .await?;

        if let Some(content) = recoverable_assistant_content(&operation) {
            let coach_message = self
                .append_recovered_coach_message(user_id, workout_id, &operation, content)
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

        self.fail_invalid_provider_transcript_recovery(&operation)
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
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let mut operation = operation.clone();
        let public_tool_calls: Vec<_> = operation
            .provider_transcript
            .iter()
            .filter(|message| message.role == crate::domain::llm::LlmMessageRole::Assistant)
            .flat_map(|message| message.tool_calls.iter())
            .map(public_tool_call_from_llm)
            .collect();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let service = self.clone();

        operation.public_tool_call_ids = materialize_public_tool_calls_idempotently(
            operation.public_tool_call_ids.clone(),
            &public_tool_calls,
            |tool_call_id| {
                let service = service.clone();
                let user_id = user_id.clone();
                let workout_id = workout_id.clone();
                let tool_call_id = tool_call_id.to_string();
                async move {
                    service
                        .recovery_tool_call_already_materialized(
                            &user_id,
                            &workout_id,
                            &tool_call_id,
                        )
                        .await
                }
            },
            |tool_call| {
                let service = service.clone();
                let user_id = user_id.clone();
                let workout_id = workout_id.clone();
                async move {
                    service
                        .append_tool_message(&user_id, &workout_id, tool_call)
                        .await
                        .map(|_| ())
                }
            },
        )
        .await?;

        Ok(operation)
    }

    async fn recovery_tool_call_already_materialized(
        &self,
        user_id: &str,
        workout_id: &str,
        tool_call_id: &str,
    ) -> Result<bool, WorkoutSummaryError> {
        self.tool_call_is_already_materialized(user_id, workout_id, tool_call_id)
            .await
    }

    async fn append_recovered_coach_message(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        content: String,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let coach_message_id = operation.reply_message_id.clone().ok_or_else(|| {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::{
        llm::{
            LlmCacheUsage, LlmChatMessage, LlmProvider, LlmTokenUsage, LlmToolCall,
            PendingLlmReplyCheckpoint,
        },
        workout_summary::{CoachReplyOperation, MockWorkoutCoach, WorkoutSummaryService},
    };

    use crate::domain::workout_summary::service::tests::{
        ExistingMessageSummaryRepository, FixedClock, FixedIds, RecordingReplyOperations,
        StubReplyOperations,
    };

    #[tokio::test]
    async fn recovery_materialization_does_not_duplicate_tool_messages() {
        let repository = ExistingMessageSummaryRepository::with_messages(Vec::new());
        let service = WorkoutSummaryService::with_coach(
            repository.clone(),
            StubReplyOperations,
            FixedClock,
            FixedIds,
            Arc::new(MockWorkoutCoach),
        );
        let operation = CoachReplyOperation::pending(
            "user-1".to_string(),
            "workout-1".to_string(),
            "message-1".to_string(),
            Some("workout-summary:user-1:workout-1".to_string()),
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

        service
            .materialize_missing_recovery_tool_messages("user-1", "workout-1", &operation)
            .await
            .expect("first recovery materialization should succeed");
        service
            .materialize_missing_recovery_tool_messages("user-1", "workout-1", &operation)
            .await
            .expect("second recovery materialization should stay idempotent");

        assert_eq!(
            repository.appended_message_ids(),
            vec!["tool-1".to_string(), "tool-2".to_string()]
        );
    }

    #[tokio::test]
    async fn recovery_persists_materialized_tool_call_ids_before_completion() {
        let repository = ExistingMessageSummaryRepository::with_messages(Vec::new());
        let reply_operations = RecordingReplyOperations::default();
        let service = WorkoutSummaryService::with_coach(
            repository.clone(),
            reply_operations.clone(),
            FixedClock,
            FixedIds,
            Arc::new(MockWorkoutCoach),
        );
        let operation = CoachReplyOperation::pending(
            "user-1".to_string(),
            "workout-1".to_string(),
            "message-1".to_string(),
            Some("workout-summary:user-1:workout-1".to_string()),
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
                "Recovered coach reply",
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

        let reply = service
            .try_recover_pending_operation("user-1", "workout-1", "message-1", &operation)
            .await
            .expect("recovery should succeed")
            .expect("recovery should replay the persisted reply");

        let persisted = reply_operations
            .last_upserted_operation()
            .expect("recovery should persist completed operation");

        assert_eq!(
            persisted.status,
            crate::domain::llm::LlmReplyOperationStatus::Completed
        );
        assert_eq!(
            persisted.public_tool_call_ids,
            vec!["tool-1".to_string(), "tool-2".to_string()]
        );
        assert_eq!(reply.coach_message.id, "coach-message-1".to_string());
        assert_eq!(
            repository.appended_message_ids(),
            vec![
                "tool-1".to_string(),
                "tool-2".to_string(),
                "coach-message-1".to_string()
            ]
        );
    }
}
