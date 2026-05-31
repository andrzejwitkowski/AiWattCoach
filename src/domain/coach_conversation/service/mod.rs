use std::sync::Arc;

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{LlmChatPort, LlmContextCacheRepository, UserLlmConfigProvider},
    llm_tools::{GetSelectedWorkoutDataPort, UpdatePlannedWorkoutDataPort},
    settings::UserSettingsUseCases,
    training_context::TrainingContextBuilder,
};

use super::{
    BoxFuture, CoachConversation, CoachConversationError, CoachConversationMessage,
    CoachConversationMessageRepository, CoachConversationReply,
    CoachConversationReplyOperationRepository, CoachConversationRepository,
    PersistedConversationUserMessage, SendConversationMessageResult,
};

mod internals;
mod request;
mod scheduler;
pub mod transcript;
mod use_cases;

pub use scheduler::{
    coach_conversation_reply_task_handler, SchedulerBackedCoachConversationService,
};

const POST_PROVIDER_WRITE_ATTEMPTS: usize = 2;
pub(super) const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

pub trait CoachConversationUseCases: Send + Sync {
    fn get_or_create_active_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn start_new_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn get_calendar_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn send_calendar_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>>;

    fn append_calendar_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>>;

    fn generate_calendar_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>>;
}

#[derive(Clone)]
pub struct SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: CoachConversationRepository + Clone,
    Messages: CoachConversationMessageRepository + Clone,
    Ops: CoachConversationReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    conversations: Conversations,
    messages: Messages,
    reply_operations: Ops,
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    settings_service: Option<Arc<dyn UserSettingsUseCases>>,
    context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
    planned_workout_update_port: Option<Arc<dyn UpdatePlannedWorkoutDataPort>>,
    clock: Time,
    ids: Ids,
}

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: CoachConversationRepository + Clone,
    Messages: CoachConversationMessageRepository + Clone,
    Ops: CoachConversationReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversations: Conversations,
        messages: Messages,
        reply_operations: Ops,
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            conversations,
            messages,
            reply_operations,
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
            settings_service: None,
            context_cache_repository: None,
            data_port: None,
            planned_workout_update_port: None,
            clock,
            ids,
        }
    }

    pub fn with_settings_service(
        mut self,
        settings_service: Arc<dyn UserSettingsUseCases>,
    ) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    pub fn with_context_cache_repository(
        mut self,
        context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.context_cache_repository = Some(context_cache_repository);
        self
    }

    pub fn with_data_port(mut self, data_port: Arc<dyn GetSelectedWorkoutDataPort>) -> Self {
        self.data_port = Some(data_port);
        self
    }

    pub fn with_planned_workout_update_port(
        mut self,
        planned_workout_update_port: Arc<dyn UpdatePlannedWorkoutDataPort>,
    ) -> Self {
        self.planned_workout_update_port = Some(planned_workout_update_port);
        self
    }
}
