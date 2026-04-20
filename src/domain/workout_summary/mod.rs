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
pub use service::{
    spawn_workout_summary_coach_reply_task_runner, workout_summary_coach_reply_task_handler,
    CompletedWorkoutTargetUseCases, LatestCompletedActivityUseCases, SaveSummaryResult,
    SaveWorkflowResult, SaveWorkflowStatus, SchedulerBackedWorkoutSummaryService,
    WorkoutSummaryService, WorkoutSummaryUseCases,
};
