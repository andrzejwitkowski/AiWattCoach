use std::sync::Arc;

use crate::domain::coach_conversation::{
    BoxFuture, CoachConversation, CoachConversationError, CoachConversationMessage,
    CoachConversationReply, CoachConversationUseCases, PersistedConversationUserMessage,
    SendConversationMessageResult,
};

pub trait CalendarCoachUseCases: Send + Sync {
    fn get_current_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn start_new_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn get_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn send_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>>;

    fn append_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>>;

    fn generate_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>>;
}

#[derive(Clone)]
pub struct SharedCalendarCoachService<Inner>
where
    Inner: CoachConversationUseCases + 'static,
{
    inner: Arc<Inner>,
}

impl<Inner> SharedCalendarCoachService<Inner>
where
    Inner: CoachConversationUseCases + 'static,
{
    pub fn new(inner: Arc<Inner>) -> Self {
        Self { inner }
    }
}

impl<Inner> CalendarCoachUseCases for SharedCalendarCoachService<Inner>
where
    Inner: CoachConversationUseCases + 'static,
{
    fn get_current_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        self.inner
            .get_or_create_active_calendar_conversation(user_id)
    }

    fn start_new_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        self.inner.start_new_calendar_conversation(user_id)
    }

    fn get_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        self.inner
            .get_calendar_conversation(user_id, conversation_id)
    }

    fn send_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>> {
        self.inner
            .send_calendar_message(user_id, conversation_id, content)
    }

    fn append_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>> {
        self.inner
            .append_calendar_user_message(user_id, conversation_id, content)
    }

    fn generate_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>> {
        self.inner
            .generate_calendar_reply(user_id, conversation_id, user_message_id)
    }
}
