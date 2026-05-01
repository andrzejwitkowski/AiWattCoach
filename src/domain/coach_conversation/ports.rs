use std::{future::Future, pin::Pin};

use super::{
    CoachConversation, CoachConversationError, CoachConversationMessage,
    CoachConversationReplyClaimResult, CoachConversationReplyOperation, CoachConversationStatus,
    CoachConversationSurface,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait CoachConversationRepository: Send + Sync + 'static {
    fn find_active_by_user_id_and_surface(
        &self,
        user_id: &str,
        surface: &CoachConversationSurface,
    ) -> BoxFuture<Result<Option<CoachConversation>, CoachConversationError>>;

    fn find_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversation>, CoachConversationError>>;

    fn create(
        &self,
        conversation: CoachConversation,
    ) -> BoxFuture<Result<CoachConversation, CoachConversationError>>;

    fn update_status(
        &self,
        user_id: &str,
        conversation_id: &str,
        status: CoachConversationStatus,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), CoachConversationError>>;

    fn touch_updated_at(
        &self,
        user_id: &str,
        conversation_id: &str,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), CoachConversationError>>;
}

pub trait CoachConversationMessageRepository: Send + Sync + 'static {
    fn list_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<Vec<CoachConversationMessage>, CoachConversationError>>;

    fn append(
        &self,
        message: CoachConversationMessage,
    ) -> BoxFuture<Result<CoachConversationMessage, CoachConversationError>>;

    fn find_by_user_id_and_conversation_id_and_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversationMessage>, CoachConversationError>>;
}

pub trait CoachConversationReplyOperationRepository: Send + Sync + 'static {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversationReplyOperation>, CoachConversationError>>;

    fn claim_pending(
        &self,
        operation: CoachConversationReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachConversationReplyClaimResult, CoachConversationError>>;

    fn upsert(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> BoxFuture<Result<CoachConversationReplyOperation, CoachConversationError>>;
}
