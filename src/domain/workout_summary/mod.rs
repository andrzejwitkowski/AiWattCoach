mod coach;
mod coach_output;
mod model;
mod ports;
mod prompt;
mod save_completion_port;
mod service;
mod test_support;

pub use coach::{MockWorkoutCoach, WorkoutCoach};
pub use coach_output::{
    parse_coach_reply, workout_summary_coach_reply_json_schema, ParsedCoachReply,
};
pub use model::{
    validate_message_content, validate_rpe, CoachQuestion, CoachReply, CoachReplyClaimResult,
    CoachReplyOperation, CoachReplyOperationFailureKind, CoachReplyOperationStatus,
    CompletedCoachReply, ConversationMessage, MessageRole, PendingCoachReplyCheckpoint,
    PersistedUserMessage, PublicToolCall, SendMessageResult, WorkoutRecap, WorkoutSummary,
    WorkoutSummaryError,
};
pub use ports::{BoxFuture, CoachReplyOperationRepository, WorkoutSummaryRepository};
pub use prompt::{
    assemble_workout_summary_coach_request, build_conversation, build_stable_context,
    build_volatile_context, workout_coach_system_prompt, WorkoutSummaryCoachPromptInput,
    ADMIN_PREVIEW_USER_MESSAGE,
};
pub use save_completion_port::{NoopSaveWorkflowCompletionPort, SaveWorkflowCompletionPort};
pub use service::{
    workout_summary_coach_reply_task_handler, CompletedWorkoutAliasScope,
    CompletedWorkoutTargetUseCases, LatestCompletedActivityUseCases,
    ResolvedCompletedWorkoutTarget, SaveSummaryResult, SaveWorkflowResult, SaveWorkflowStatus,
    SchedulerBackedWorkoutSummaryService, WorkoutSummaryGetOptions, WorkoutSummaryListOptions,
    WorkoutSummaryService, WorkoutSummaryUseCases,
};
pub(crate) use service::{
    COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS, COACH_REPLY_LEASE_DURATION_SECONDS,
    COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS,
};
#[doc(hidden)]
pub use test_support::{coach_reply_json, coach_reply_json_with_question};
