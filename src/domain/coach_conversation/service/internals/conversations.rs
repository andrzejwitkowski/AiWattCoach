use super::super::*;
use crate::domain::coach_conversation::{
    CoachConversationFocus, CoachConversationMessageRole, CoachConversationReplyOperation,
    CoachConversationStatus, CoachConversationSurface,
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
    pub(in super::super) async fn get_existing_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        self.conversations
            .find_by_user_id_and_conversation_id(user_id, conversation_id)
            .await?
            .ok_or(CoachConversationError::NotFound)
    }

    pub(in super::super) async fn get_existing_active_conversation(
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

    pub(in super::super) async fn list_messages(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Vec<CoachConversationMessage>, CoachConversationError> {
        self.messages
            .list_by_user_id_and_conversation_id(user_id, conversation_id)
            .await
    }

    pub(in super::super) async fn create_calendar_conversation(
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

    pub(in super::super) async fn archive_active_calendar_conversation_if_present(
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

    pub(in super::super) async fn load_persisted_user_message(
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

    pub(in super::super) fn build_pending_reply_operation(
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

    pub(in super::super) async fn get_completed_reply(
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
}
