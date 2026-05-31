mod model;
mod ports;
mod prompt;
mod service;

pub use model::{
    validate_conversation_message_content, CoachConversation, CoachConversationError,
    CoachConversationFocus, CoachConversationMessage, CoachConversationMessageRole,
    CoachConversationReply, CoachConversationReplyClaimResult, CoachConversationReplyOperation,
    CoachConversationReplyOperationFailureKind, CoachConversationReplyOperationStatus,
    CoachConversationStatus, CoachConversationSurface, CompletedCoachConversationReply,
    PendingCoachConversationReplyCheckpoint, PersistedConversationUserMessage,
    SendConversationMessageResult,
};
pub use ports::{
    BoxFuture, CoachConversationMessageRepository, CoachConversationReplyOperationRepository,
    CoachConversationRepository,
};
pub use prompt::{assemble_calendar_coach_request, CalendarCoachPromptInput};
pub use service::{
    coach_conversation_reply_task_handler, CoachConversationUseCases,
    SchedulerBackedCoachConversationService, SharedCoachConversationService,
};
