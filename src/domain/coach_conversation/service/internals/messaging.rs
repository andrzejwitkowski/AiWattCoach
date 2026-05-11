use crate::domain::{
    coach_conversation::{
        validate_conversation_message_content, CoachConversationMessageRole,
        CoachConversationReplyOperation,
    },
    settings::{SettingsError, UserSettings},
    workout_summary::PublicToolCall,
};

use super::super::*;
use crate::domain::coach_conversation::CoachConversationStatus;

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    pub(in super::super) async fn append_message(
        &self,
        conversation: &CoachConversation,
        role: CoachConversationMessageRole,
        content: String,
        message_id: Option<String>,
        tool_call: Option<PublicToolCall>,
        reasoning_content: Option<String>,
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
                reasoning_content,
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

    pub(super) async fn append_tool_message(
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
            None,
        )
        .await
    }

    pub(in super::super) async fn materialize_public_tool_messages(
        &self,
        conversation: &CoachConversation,
        operation: CoachConversationReplyOperation,
        public_tool_calls: &[PublicToolCall],
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let mut operation = operation;

        for tool_call in public_tool_calls {
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

            self.append_tool_message(conversation, tool_call.clone())
                .await?;
            operation.public_tool_call_ids.push(tool_call.id.clone());
        }

        Ok(operation)
    }

    pub(super) async fn tool_message_already_materialized(
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
