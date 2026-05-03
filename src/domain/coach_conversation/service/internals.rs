use std::time::Duration;

use crate::domain::{
    llm::{LlmChatMessage, LlmChatResponse, LlmError, LlmMessageRole},
    settings::{SettingsError, UserSettings},
    workout_summary::PublicToolCall,
};

use super::{
    super::{
        validate_conversation_message_content, CoachConversation, CoachConversationError,
        CoachConversationFocus, CoachConversationMessage, CoachConversationMessageRole,
        CoachConversationReply, CoachConversationReplyOperation, CoachConversationStatus,
        CoachConversationSurface,
    },
    transcript::merge_hidden_transcript_entries,
    SharedCoachConversationService, POST_PROVIDER_WRITE_ATTEMPTS,
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
    pub(super) async fn get_existing_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        self.conversations
            .find_by_user_id_and_conversation_id(user_id, conversation_id)
            .await?
            .ok_or(CoachConversationError::NotFound)
    }

    pub(super) async fn get_existing_active_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        let conversation = self
            .get_existing_conversation(user_id, conversation_id)
            .await?;
        if conversation.status == CoachConversationStatus::Archived {
            return Err(CoachConversationError::Archived);
        }
        Ok(conversation)
    }

    pub(super) async fn list_messages(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Vec<CoachConversationMessage>, CoachConversationError> {
        self.messages
            .list_by_user_id_and_conversation_id(user_id, conversation_id)
            .await
    }

    pub(super) async fn create_calendar_conversation(
        &self,
        user_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        self.conversations
            .create(CoachConversation::new(
                self.ids.new_id("conversation"),
                user_id.to_string(),
                CoachConversationSurface::Calendar,
                CoachConversationFocus::Overview,
                self.clock.now_epoch_seconds(),
            ))
            .await
    }

    pub(super) async fn archive_active_calendar_conversation_if_present(
        &self,
        user_id: &str,
    ) -> Result<(), CoachConversationError> {
        let Some(existing) = self
            .conversations
            .find_active_by_user_id_and_surface(user_id, &CoachConversationSurface::Calendar)
            .await?
        else {
            return Ok(());
        };

        self.conversations
            .update_status(
                user_id,
                &existing.conversation_id,
                CoachConversationStatus::Archived,
                self.clock.now_epoch_seconds(),
            )
            .await
    }

    async fn ensure_availability_configured_for_coach(
        &self,
        user_id: &str,
    ) -> Result<(), CoachConversationError> {
        let Some(settings_service) = &self.settings_service else {
            return Ok(());
        };

        let settings = settings_service
            .find_settings(user_id)
            .await
            .map_err(map_settings_error)?
            .unwrap_or_else(|| {
                UserSettings::new_defaults(user_id.to_string(), self.clock.now_epoch_seconds())
            });

        if settings.availability.is_configured() {
            Ok(())
        } else {
            Err(CoachConversationError::Validation(
                "availability must be configured before chatting with coach".to_string(),
            ))
        }
    }

    pub(super) async fn append_message(
        &self,
        conversation: &CoachConversation,
        role: CoachConversationMessageRole,
        content: String,
        message_id: Option<String>,
        tool_call: Option<PublicToolCall>,
    ) -> Result<CoachConversationMessage, CoachConversationError> {
        if conversation.status == CoachConversationStatus::Archived {
            return Err(CoachConversationError::Archived);
        }

        let content = validate_conversation_message_content(&content)?;
        if matches!(role, CoachConversationMessageRole::User) {
            self.ensure_availability_configured_for_coach(&conversation.user_id)
                .await?;
        }

        let message = self
            .messages
            .append(CoachConversationMessage {
                id: message_id.unwrap_or_else(|| self.ids.new_id("message")),
                conversation_id: conversation.conversation_id.clone(),
                user_id: conversation.user_id.clone(),
                role,
                content,
                tool_call,
                created_at_epoch_seconds: self.clock.now_epoch_seconds(),
            })
            .await?;

        self.conversations
            .touch_updated_at(
                &conversation.user_id,
                &conversation.conversation_id,
                self.clock.now_epoch_seconds(),
            )
            .await?;

        Ok(message)
    }

    async fn append_tool_message(
        &self,
        conversation: &CoachConversation,
        tool_call: PublicToolCall,
    ) -> Result<CoachConversationMessage, CoachConversationError> {
        self.append_message(
            conversation,
            CoachConversationMessageRole::Tool,
            format!("Tool call: {}", tool_call.name),
            Some(tool_call.id.clone()),
            Some(tool_call),
        )
        .await
    }

    async fn replace_hidden_transcript(
        &self,
        conversation: &CoachConversation,
        hidden_transcript: Vec<LlmChatMessage>,
    ) -> Result<(), CoachConversationError> {
        self.conversations
            .replace_hidden_transcript(
                &conversation.user_id,
                &conversation.conversation_id,
                hidden_transcript,
                self.clock.now_epoch_seconds(),
            )
            .await
    }

    pub(super) async fn merge_hidden_transcript_with_retry(
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
            let merged = merge_hidden_transcript_entries(
                latest.hidden_transcript.clone(),
                &operation.hidden_transcript,
            );

            match self.replace_hidden_transcript(&latest, merged).await {
                Ok(()) => {
                    if attempt > 1 {
                        tracing::info!(
                            conversation_id = %conversation.conversation_id,
                            user_message_id = %operation.user_message_id,
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            write_label,
                            "recovered hidden transcript write after retry"
                        );
                    }
                    return Ok(());
                }
                Err(error @ CoachConversationError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    tracing::warn!(
                        conversation_id = %conversation.conversation_id,
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
            CoachConversationError::Repository(
                "hidden transcript write failed without error".to_string(),
            )
        }))
    }

    pub(super) async fn materialize_public_tool_messages(
        &self,
        conversation: &CoachConversation,
        operation: CoachConversationReplyOperation,
        response: &LlmChatResponse,
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let mut operation = operation;

        for tool_call in response.tool_calls() {
            if operation
                .public_tool_call_ids
                .iter()
                .any(|id| id == &tool_call.id)
            {
                continue;
            }

            if self
                .tool_message_already_materialized(conversation, &tool_call.id)
                .await?
            {
                operation.public_tool_call_ids.push(tool_call.id.clone());
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
            operation.public_tool_call_ids.push(tool_call.id.clone());
        }

        Ok(operation)
    }

    async fn tool_message_already_materialized(
        &self,
        conversation: &CoachConversation,
        tool_call_id: &str,
    ) -> Result<bool, CoachConversationError> {
        self.messages
            .find_by_user_id_and_conversation_id_and_message_id(
                &conversation.user_id,
                &conversation.conversation_id,
                tool_call_id,
            )
            .await
            .map(|message| message.is_some())
    }

    pub(super) async fn load_persisted_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> Result<CoachConversationMessage, CoachConversationError> {
        let user_message = self
            .messages
            .find_by_user_id_and_conversation_id_and_message_id(
                user_id,
                conversation_id,
                user_message_id,
            )
            .await?
            .ok_or(CoachConversationError::NotFound)?;

        if user_message.role != CoachConversationMessageRole::User {
            return Err(CoachConversationError::Validation(
                "user message must be persisted before generating coach reply".to_string(),
            ));
        }

        Ok(user_message)
    }

    pub(super) fn build_pending_reply_operation(
        &self,
        conversation: &CoachConversation,
        user_message: &CoachConversationMessage,
    ) -> CoachConversationReplyOperation {
        let now = self.clock.now_epoch_seconds();
        CoachConversationReplyOperation::pending(
            conversation.user_id.clone(),
            conversation.conversation_id.clone(),
            user_message.id.clone(),
            Some(format!(
                "calendar-coach:{}:{}",
                conversation.user_id,
                conversation.focus.cache_scope_suffix()
            )),
            self.ids.new_id("message"),
            now,
        )
    }

    pub(super) async fn get_completed_reply(
        &self,
        conversation: &CoachConversation,
        operation: CoachConversationReplyOperation,
    ) -> Result<CoachConversationReply, CoachConversationError> {
        let coach_message_id = operation.coach_message_id.ok_or_else(|| {
            CoachConversationError::Repository(
                "completed coach reply operation missing coach message id".to_string(),
            )
        })?;
        let coach_message = self
            .messages
            .find_by_user_id_and_conversation_id_and_message_id(
                &conversation.user_id,
                &conversation.conversation_id,
                &coach_message_id,
            )
            .await?
            .ok_or(CoachConversationError::NotFound)?;
        let messages = self
            .list_messages(&conversation.user_id, &conversation.conversation_id)
            .await?;

        Ok(CoachConversationReply {
            conversation: conversation.clone(),
            messages,
            coach_message,
            athlete_summary_was_regenerated: false,
        })
    }

    pub(super) fn map_existing_llm_failure(
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

    pub(super) async fn persist_post_provider_operation(
        &self,
        operation: CoachConversationReplyOperation,
        write_label: &'static str,
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            match self.reply_operations.upsert(operation.clone()).await {
                Ok(saved) => {
                    if attempt > 1 {
                        tracing::info!(
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

                    tracing::warn!(
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

    pub(super) async fn try_recover_pending_operation(
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

        if operation.hidden_transcript.is_empty() {
            return Ok(None);
        }

        self.replay_persisted_reply_from_hidden_transcript(conversation, operation)
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

    async fn replay_persisted_reply_from_hidden_transcript(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<Option<CoachConversationReply>, CoachConversationError> {
        if let Err(error) = self
            .merge_hidden_transcript_with_retry(
                conversation,
                operation,
                "recover_hidden_transcript",
            )
            .await
        {
            let llm_error = LlmError::Internal(format!(
                "failed to persist hidden transcript during recovery: {error}"
            ));
            let failed = operation.mark_failed(&llm_error, self.clock.now_epoch_seconds());
            self.persist_post_provider_operation(
                failed,
                "persist_failed_hidden_transcript_recovery",
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
        for transcript_message in &operation.hidden_transcript {
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

fn map_settings_error(error: SettingsError) -> CoachConversationError {
    match error {
        SettingsError::Repository(message) => CoachConversationError::Repository(message),
        SettingsError::Unauthenticated => {
            CoachConversationError::Validation("authentication is required".to_string())
        }
        SettingsError::Validation(message) => CoachConversationError::Validation(message),
    }
}

fn recovered_assistant_reply_text(operation: &CoachConversationReplyOperation) -> Option<String> {
    operation
        .hidden_transcript
        .iter()
        .rev()
        .find(|message| message.role == LlmMessageRole::Assistant)
        .map(|message| message.content.clone())
        .filter(|content| !content.trim().is_empty())
}
