mod coach;
mod model;
mod ports;
mod service;

pub use coach::{MockWorkoutCoach, WorkoutCoach};
pub use model::{
    validate_message_content, validate_rpe, CoachReply, CoachReplyClaimResult, CoachReplyOperation,
    CoachReplyOperationFailureKind, CoachReplyOperationStatus, CompletedCoachReply,
    ConversationMessage, MessageRole, PendingCoachReplyCheckpoint, PersistedUserMessage,
    SendMessageResult, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
};
pub use ports::{BoxFuture, CoachReplyOperationRepository, WorkoutSummaryRepository};
pub use service::{WorkoutSummaryService, WorkoutSummaryUseCases};
