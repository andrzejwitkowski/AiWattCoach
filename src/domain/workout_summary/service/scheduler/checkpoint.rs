use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedCoachReplyTaskCheckpoint {
    pub(crate) coach_message: ConversationMessage,
    pub(crate) athlete_summary_was_regenerated: bool,
}

fn completed_checkpoint_from_legacy_message(
    coach_message: ConversationMessage,
) -> CompletedCoachReplyTaskCheckpoint {
    CompletedCoachReplyTaskCheckpoint {
        coach_message,
        athlete_summary_was_regenerated: false,
    }
}

pub(crate) fn parse_terminal_coach_reply_checkpoint(
    task: &ScheduledTask,
) -> Result<Option<CompletedCoachReplyTaskCheckpoint>, WorkoutSummaryError> {
    task.checkpoint
        .clone()
        .map(|value| {
            serde_json::from_value::<CompletedCoachReplyTaskCheckpoint>(value.clone())
                .or_else(|_| {
                    serde_json::from_value::<ConversationMessage>(value)
                        .map(completed_checkpoint_from_legacy_message)
                })
                .map_err(|error| {
                    WorkoutSummaryError::Repository(format!(
                        "invalid completed workout summary coach reply checkpoint: {error}"
                    ))
                })
        })
        .transpose()
}

pub(crate) fn serialize_completed_coach_reply_checkpoint(
    reply: &CoachReply,
) -> Result<serde_json::Value, WorkoutSummaryError> {
    serde_json::to_value(CompletedCoachReplyTaskCheckpoint {
        coach_message: reply.coach_message.clone(),
        athlete_summary_was_regenerated: reply.athlete_summary_was_regenerated,
    })
    .map_err(|error| {
        WorkoutSummaryError::Repository(format!(
            "failed to serialize completed coach reply checkpoint: {error}"
        ))
    })
}

pub(crate) fn parse_terminal_task_error(
    task: &ScheduledTask,
) -> Result<Option<WorkoutSummaryError>, WorkoutSummaryError> {
    task.checkpoint
        .clone()
        .map(|value| {
            serde_json::from_value::<SerializedWorkoutSummaryError>(value)
                .map(deserialize_workout_summary_error)
                .map_err(|error| {
                    WorkoutSummaryError::Repository(format!(
                        "invalid failed workout summary coach reply checkpoint: {error}"
                    ))
                })
        })
        .transpose()
}
