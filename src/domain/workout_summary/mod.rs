mod coach;
mod model;
mod ports;
mod service;

pub use coach::{MockWorkoutCoach, WorkoutCoach};
pub use model::{
    validate_message_content, validate_rpe, CoachReply, CoachReplyClaimResult, CoachReplyOperation,
    CoachReplyOperationFailureKind, CoachReplyOperationStatus, CompletedCoachReply,
    ConversationMessage, MessageRole, PendingCoachReplyCheckpoint, PersistedUserMessage,
    PublicToolCall, SendMessageResult, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
};
pub use ports::{BoxFuture, CoachReplyOperationRepository, WorkoutSummaryRepository};
pub use service::{
    workout_summary_coach_reply_task_handler, CompletedWorkoutTargetUseCases,
    LatestCompletedActivityUseCases, ResolvedCompletedWorkoutTarget, SaveSummaryResult,
    SaveWorkflowResult, SaveWorkflowStatus, SchedulerBackedWorkoutSummaryService,
    WorkoutSummaryService, WorkoutSummaryUseCases,
};
pub(crate) use service::{
    COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS, COACH_REPLY_LEASE_DURATION_SECONDS,
    COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS,
};
