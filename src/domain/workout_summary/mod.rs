mod coach;
mod model;
mod ports;
mod service;

pub use coach::MockWorkoutCoach;
pub use model::{
    validate_message_content, validate_rpe, CoachReply, ConversationMessage, MessageRole,
    PersistedUserMessage, SendMessageResult, WorkoutSummary, WorkoutSummaryError,
};
pub use ports::{BoxFuture, WorkoutSummaryRepository};
pub use service::{WorkoutSummaryService, WorkoutSummaryUseCases};
