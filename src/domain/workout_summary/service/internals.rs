use std::time::Duration;

use tracing::{info, warn};

use super::*;

pub(super) struct AppendMessageInput {
    role: MessageRole,
    content: String,
    message_id: Option<String>,
    tool_call: Option<crate::domain::workout_summary::PublicToolCall>,
    require_open_summary: bool,
}

impl AppendMessageInput {
    pub(super) fn coach(content: String, message_id: String) -> Self {
        Self {
            role: MessageRole::Coach,
            content,
            message_id: Some(message_id),
            tool_call: None,
            require_open_summary: false,
        }
    }
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(super) async fn get_existing_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    pub(super) async fn resolve_workout_summary_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<ResolvedWorkoutSummaryTarget, WorkoutSummaryError> {
        let Some(service) = &self.completed_workout_target_service else {
            let existing_summary = self
                .repository
                .find_by_user_id_and_workout_id(user_id, workout_id)
                .await?;
            return Ok(ResolvedWorkoutSummaryTarget {
                requested_workout_id: workout_id.to_string(),
                preferred_workout_id: workout_id.to_string(),
                summary_workout_id: workout_id.to_string(),
                storage_workout_id: workout_id.to_string(),
                existing_summary,
            });
        };

        let Some(resolved_target) = service
            .resolve_completed_workout_target(user_id, workout_id)
            .await?
        else {
            return Err(WorkoutSummaryError::Validation(
                "workout summary is only available for completed workouts".to_string(),
            ));
        };

        let mut candidate_workout_ids = Vec::new();
        push_unique_workout_id(&mut candidate_workout_ids, workout_id.to_string());
        push_unique_workout_id(
            &mut candidate_workout_ids,
            resolved_target.preferred_workout_id.clone(),
        );
        for equivalent_workout_id in &resolved_target.equivalent_workout_ids {
            push_unique_workout_id(&mut candidate_workout_ids, equivalent_workout_id.clone());
        }

        let mut existing_summary = None;
        let mut summary_workout_id = resolved_target.preferred_workout_id.clone();
        let mut storage_workout_id = resolved_target.preferred_workout_id.clone();
        for candidate_workout_id in candidate_workout_ids {
            if let Some(summary) = self
                .repository
                .find_by_user_id_and_workout_id(user_id, &candidate_workout_id)
                .await?
            {
                storage_workout_id = candidate_workout_id;
                summary_workout_id = summary.workout_id.clone();
                existing_summary = Some(summary);
                break;
            }
        }

        Ok(ResolvedWorkoutSummaryTarget {
            requested_workout_id: workout_id.to_string(),
            preferred_workout_id: resolved_target.preferred_workout_id,
            summary_workout_id,
            storage_workout_id,
            existing_summary,
        })
    }

    pub(super) async fn validate_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        self.resolve_workout_summary_target(user_id, workout_id)
            .await
            .map(|_| ())
    }

    pub(super) async fn append_message_with_role(
        &self,
        user_id: &str,
        workout_id: &str,
        role: MessageRole,
        content: String,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.append_message_with_role_and_id(
            user_id,
            workout_id,
            AppendMessageInput {
                role,
                content,
                message_id: None,
                tool_call: None,
                require_open_summary: true,
            },
        )
        .await
    }

    pub(super) async fn append_tool_message(
        &self,
        user_id: &str,
        workout_id: &str,
        tool_call: crate::domain::workout_summary::PublicToolCall,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.append_message_with_role_and_id(
            user_id,
            workout_id,
            AppendMessageInput {
                role: MessageRole::Tool,
                content: format!("Tool call: {}", tool_call.name),
                message_id: Some(tool_call.id.clone()),
                tool_call: Some(tool_call),
                require_open_summary: false,
            },
        )
        .await
    }

    pub(super) async fn replace_hidden_transcript(
        &self,
        user_id: &str,
        workout_id: &str,
        hidden_transcript: Vec<crate::domain::llm::LlmChatMessage>,
    ) -> Result<(), WorkoutSummaryError> {
        self.repository
            .replace_hidden_transcript(
                user_id,
                workout_id,
                hidden_transcript,
                self.clock.now_epoch_seconds(),
            )
            .await
    }

    pub(super) async fn merge_hidden_transcript_with_retry(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: &CoachReplyOperation,
        write_label: &'static str,
    ) -> Result<(), WorkoutSummaryError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            let summary = self.get_existing_summary(user_id, workout_id).await?;
            let merged = merge_hidden_transcript_entries(
                summary.hidden_transcript,
                &operation.hidden_transcript,
            );

            match self
                .replace_hidden_transcript(user_id, workout_id, merged)
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
                            "recovered hidden transcript write after retry"
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
                        "retrying hidden transcript write after repository error"
                    );
                    last_error = Some(error);
                    tokio::time::sleep(Duration::from_millis(25 * attempt as u64)).await;
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            WorkoutSummaryError::Repository(
                "hidden transcript write failed without error".to_string(),
            )
        }))
    }

    pub(super) async fn materialize_public_tool_messages(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
        response: &crate::domain::llm::LlmChatResponse,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let mut operation = operation;

        for tool_call in response.tool_calls() {
            if operation
                .public_tool_call_ids
                .iter()
                .any(|id| id == &tool_call.id)
            {
                continue;
            }

            let already_materialized = match self
                .get_message_by_id(user_id, workout_id, &tool_call.id)
                .await
            {
                Ok(_) => true,
                Err(WorkoutSummaryError::NotFound) => false,
                Err(error) => return Err(error),
            };
            if already_materialized {
                operation.public_tool_call_ids.push(tool_call.id.clone());
                continue;
            }

            self.append_tool_message(
                user_id,
                workout_id,
                crate::domain::workout_summary::PublicToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments_json: tool_call.arguments_json.clone(),
                },
            )
            .await?;
            operation.public_tool_call_ids.push(tool_call.id.clone());
        }

        Ok(operation)
    }

    pub(super) async fn ensure_availability_configured_for_coach(
        &self,
        user_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        let Some(settings_service) = &self.settings_service else {
            return Ok(());
        };

        let settings = settings_service
            .find_settings(user_id)
            .await
            .map_err(|error| match error {
                crate::domain::settings::SettingsError::Repository(message) => {
                    WorkoutSummaryError::Repository(message)
                }
                crate::domain::settings::SettingsError::Unauthenticated => {
                    WorkoutSummaryError::Validation("authentication is required".to_string())
                }
                crate::domain::settings::SettingsError::Validation(message) => {
                    WorkoutSummaryError::Validation(message)
                }
            })?
            .unwrap_or_else(|| {
                crate::domain::settings::UserSettings::new_defaults(
                    user_id.to_string(),
                    self.clock.now_epoch_seconds(),
                )
            });

        if settings.availability.is_configured() {
            Ok(())
        } else {
            Err(WorkoutSummaryError::Validation(
                "availability must be configured before chatting with coach".to_string(),
            ))
        }
    }

    pub(super) async fn append_message_with_role_and_id(
        &self,
        user_id: &str,
        workout_id: &str,
        input: AppendMessageInput,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        if input.require_open_summary && summary.saved_at_epoch_seconds.is_some() {
            return Err(WorkoutSummaryError::Locked);
        }
        if input.require_open_summary && summary.rpe.is_none() {
            return Err(WorkoutSummaryError::Validation(
                "rpe must be set before chatting with coach".to_string(),
            ));
        }
        let content = validate_message_content(&input.content)?;
        if input.require_open_summary && matches!(input.role, MessageRole::User) {
            self.ensure_availability_configured_for_coach(user_id)
                .await?;
        }
        let now = self.clock.now_epoch_seconds();
        let message = ConversationMessage {
            id: input
                .message_id
                .unwrap_or_else(|| self.ids.new_id("message")),
            role: input.role,
            content,
            tool_call: input.tool_call,
            created_at_epoch_seconds: now,
        };

        self.repository
            .append_message(user_id, workout_id, message.clone(), now)
            .await?;

        Ok(message)
    }

    pub(super) async fn get_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.repository
            .find_message_by_id(user_id, workout_id, message_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    pub(super) async fn get_completed_reply(
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

    pub(super) fn present_summary(
        &self,
        mut summary: WorkoutSummary,
        requested_workout_id: &str,
    ) -> WorkoutSummary {
        summary.workout_id = requested_workout_id.to_string();
        summary
    }

    pub(super) fn present_persisted_user_message(
        &self,
        mut persisted: PersistedUserMessage,
        requested_workout_id: &str,
    ) -> PersistedUserMessage {
        persisted.summary = self.present_summary(persisted.summary, requested_workout_id);
        persisted
    }

    pub(super) fn present_coach_reply(
        &self,
        mut reply: CoachReply,
        requested_workout_id: &str,
    ) -> CoachReply {
        reply.summary = self.present_summary(reply.summary, requested_workout_id);
        reply
    }

    pub(super) fn present_save_summary_result(
        &self,
        mut result: SaveSummaryResult,
        requested_workout_id: &str,
    ) -> SaveSummaryResult {
        result.summary = self.present_summary(result.summary, requested_workout_id);
        result
    }

    pub(super) fn map_existing_llm_failure(
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

    pub(super) async fn try_recover_pending_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        if let Some(existing_coach_message_id) = operation.coach_message_id.clone() {
            if let Some(existing_coach_message) = self
                .repository
                .find_message_by_id(user_id, workout_id, &existing_coach_message_id)
                .await?
            {
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
                return Ok(Some(CoachReply {
                    summary,
                    coach_message: existing_coach_message,
                    athlete_summary_was_regenerated: false,
                }));
            }
        }

        if !operation.hidden_transcript.is_empty() {
            if let Err(error) = self
                .merge_hidden_transcript_with_retry(
                    user_id,
                    workout_id,
                    operation,
                    "recover_hidden_transcript",
                )
                .await
            {
                let llm_error = crate::domain::llm::LlmError::Internal(format!(
                    "failed to persist hidden transcript during recovery: {error}"
                ));
                let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
                self.persist_post_provider_operation(
                    failed,
                    "persist_failed_hidden_transcript_recovery",
                )
                .await?;
                return Err(WorkoutSummaryError::Llm(llm_error));
            }

            for transcript_message in &operation.hidden_transcript {
                if transcript_message.role != crate::domain::llm::LlmMessageRole::Assistant {
                    continue;
                }

                for tool_call in &transcript_message.tool_calls {
                    let existing_tool_message = match self
                        .get_message_by_id(user_id, workout_id, &tool_call.id)
                        .await
                    {
                        Ok(message) => Some(message),
                        Err(WorkoutSummaryError::NotFound) => None,
                        Err(error) => return Err(error),
                    };

                    if operation
                        .public_tool_call_ids
                        .iter()
                        .any(|id| id == &tool_call.id)
                        || existing_tool_message.is_some()
                    {
                        continue;
                    }

                    self.append_tool_message(
                        user_id,
                        workout_id,
                        crate::domain::workout_summary::PublicToolCall {
                            id: tool_call.id.clone(),
                            name: tool_call.name.clone(),
                            arguments_json: tool_call.arguments_json.clone(),
                        },
                    )
                    .await?;
                }
            }

            if let Some(content) = operation
                .hidden_transcript
                .iter()
                .rev()
                .find(|message| message.role == crate::domain::llm::LlmMessageRole::Assistant)
                .map(|message| message.content.clone())
                .filter(|content| !content.trim().is_empty())
            {
                let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "pending coach reply operation missing reserved coach message id"
                            .to_string(),
                    )
                })?;
                let coach_message = self
                    .append_message_with_role_and_id(
                        user_id,
                        workout_id,
                        AppendMessageInput {
                            role: MessageRole::Coach,
                            content,
                            message_id: Some(coach_message_id),
                            tool_call: None,
                            require_open_summary: false,
                        },
                    )
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

            let error = crate::domain::llm::LlmError::InvalidResponse(
                "assistant reply missing final text message".to_string(),
            );
            let failed = operation.mark_failed(&error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(failed, "replay_invalid_coach_reply")
                .await?;
            return Err(WorkoutSummaryError::Llm(error));
        }

        Ok(None)
    }

    pub(super) async fn persist_post_provider_operation(
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

    pub(super) async fn ensure_athlete_summary(
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
}

fn merge_hidden_transcript_entries(
    mut existing: Vec<crate::domain::llm::LlmChatMessage>,
    pending: &[crate::domain::llm::LlmChatMessage],
) -> Vec<crate::domain::llm::LlmChatMessage> {
    for entry in pending {
        if !existing.contains(entry) {
            existing.push(entry.clone());
        }
    }

    existing
}

fn push_unique_workout_id(workout_ids: &mut Vec<String>, workout_id: String) {
    if !workout_ids.contains(&workout_id) {
        workout_ids.push(workout_id);
    }
}
