use std::sync::Arc;

use crate::domain::llm::next_provider_transcript_updated_at_epoch_seconds;

use super::super::*;
use super::{map_settings_error, AppendMessageInput};

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(in super::super) async fn append_message_with_role(
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

    pub(in super::super) async fn append_tool_message(
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

    pub(in super::super) async fn replace_provider_transcript(
        &self,
        user_id: &str,
        workout_id: &str,
        expected_updated_at_epoch_seconds: i64,
        provider_transcript: Vec<crate::domain::llm::LlmChatMessage>,
    ) -> Result<(), WorkoutSummaryError> {
        let updated_at_epoch_seconds = next_provider_transcript_updated_at_epoch_seconds(
            expected_updated_at_epoch_seconds,
            self.clock.now_epoch_seconds(),
        );
        self.repository
            .replace_provider_transcript(
                user_id,
                workout_id,
                provider_transcript,
                expected_updated_at_epoch_seconds,
                updated_at_epoch_seconds,
            )
            .await
    }

    pub(in super::super) async fn materialize_public_tool_messages(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
        public_tool_calls: &[crate::domain::workout_summary::PublicToolCall],
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let mut operation = operation;

        for tool_call in public_tool_calls {
            if self
                .tool_call_is_already_materialized(user_id, workout_id, &operation, &tool_call.id)
                .await?
            {
                if !operation
                    .public_tool_call_ids
                    .iter()
                    .any(|id| id == &tool_call.id)
                {
                    operation.public_tool_call_ids.push(tool_call.id.clone());
                }
                continue;
            }

            self.append_tool_message(user_id, workout_id, tool_call.clone())
                .await?;
            operation.public_tool_call_ids.push(tool_call.id.clone());
        }

        Ok(operation)
    }

    pub(in super::super) async fn ensure_availability_configured_for_coach(
        &self,
        user_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        if !self.availability_is_configured(user_id).await? {
            return Err(WorkoutSummaryError::Validation(
                "availability must be configured before chatting with coach".to_string(),
            ));
        }

        Ok(())
    }

    pub(in super::super) async fn append_message_with_role_and_id(
        &self,
        user_id: &str,
        workout_id: &str,
        input: AppendMessageInput,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        let content = self
            .validate_append_message(user_id, &summary, &input)
            .await?;
        let message = self.build_conversation_message(input, content);
        let now = message.created_at_epoch_seconds;

        self.repository
            .append_message(user_id, workout_id, message.clone(), now)
            .await?;

        Ok(message)
    }

    pub(in super::super) async fn get_message_by_id(
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

    async fn availability_is_configured(&self, user_id: &str) -> Result<bool, WorkoutSummaryError> {
        let Some(settings_service) = &self.settings_service else {
            return Ok(true);
        };

        Ok(self
            .find_coach_settings(settings_service, user_id)
            .await?
            .availability
            .is_configured())
    }

    async fn find_coach_settings(
        &self,
        settings_service: &Arc<dyn crate::domain::settings::UserSettingsUseCases>,
        user_id: &str,
    ) -> Result<crate::domain::settings::UserSettings, WorkoutSummaryError> {
        Ok(settings_service
            .find_settings(user_id)
            .await
            .map_err(map_settings_error)?
            .unwrap_or_else(|| {
                crate::domain::settings::UserSettings::new_defaults(
                    user_id.to_string(),
                    self.clock.now_epoch_seconds(),
                )
            }))
    }

    async fn validate_append_message(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        input: &AppendMessageInput,
    ) -> Result<String, WorkoutSummaryError> {
        if input.require_open_summary {
            ensure_summary_accepts_manual_messages(summary)?;
        }

        let content = validate_message_content(&input.content)?;
        if input.require_open_summary && matches!(input.role, MessageRole::User) {
            self.ensure_availability_configured_for_coach(user_id)
                .await?;
        }

        Ok(content)
    }

    fn build_conversation_message(
        &self,
        input: AppendMessageInput,
        content: String,
    ) -> ConversationMessage {
        let now = self.clock.now_epoch_seconds();

        ConversationMessage {
            id: input
                .message_id
                .unwrap_or_else(|| self.ids.new_id("message")),
            role: input.role,
            content,
            tool_call: input.tool_call,
            created_at_epoch_seconds: now,
        }
    }

    async fn tool_call_is_already_materialized(
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
}

fn ensure_summary_accepts_manual_messages(
    summary: &WorkoutSummary,
) -> Result<(), WorkoutSummaryError> {
    if summary.saved_at_epoch_seconds.is_some() {
        return Err(WorkoutSummaryError::Locked);
    }
    if summary.rpe.is_none() {
        return Err(WorkoutSummaryError::Validation(
            "rpe must be set before chatting with coach".to_string(),
        ));
    }

    Ok(())
}
