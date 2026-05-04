use super::super::*;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(in super::super) fn present_summary(
        &self,
        mut summary: WorkoutSummary,
        requested_workout_id: &str,
    ) -> WorkoutSummary {
        summary.workout_id = requested_workout_id.to_string();
        summary
    }

    pub(in super::super) fn present_persisted_user_message(
        &self,
        mut persisted: PersistedUserMessage,
        requested_workout_id: &str,
    ) -> PersistedUserMessage {
        persisted.summary = self.present_summary(persisted.summary, requested_workout_id);
        persisted
    }

    pub(in super::super) fn present_coach_reply(
        &self,
        mut reply: CoachReply,
        requested_workout_id: &str,
    ) -> CoachReply {
        reply.summary = self.present_summary(reply.summary, requested_workout_id);
        reply
    }

    pub(in super::super) fn present_save_summary_result(
        &self,
        mut result: SaveSummaryResult,
        requested_workout_id: &str,
    ) -> SaveSummaryResult {
        result.summary = self.present_summary(result.summary, requested_workout_id);
        result
    }
}
