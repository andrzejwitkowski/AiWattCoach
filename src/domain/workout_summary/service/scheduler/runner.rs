use tracing::warn;

use super::super::STALE_PENDING_TIMEOUT_SECONDS;
use super::checkpoint::serialize_completed_coach_reply_checkpoint;
use super::*;

fn llm_error_is_retryable(error: &LlmError) -> bool {
    error.is_retryable()
}

struct WorkoutSummaryCoachReplyTaskExecutor<Base> {
    base: Arc<Base>,
}

impl<Base> WorkoutSummaryCoachReplyTaskExecutor<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    fn map_task_failure(error: WorkoutSummaryError) -> TaskRunOutcome {
        let (retryable, retry_delay_seconds) = match &error {
            WorkoutSummaryError::Llm(llm_error) => (llm_error_is_retryable(llm_error), None),
            WorkoutSummaryError::Repository(_) => (true, None),
            WorkoutSummaryError::ReplyAlreadyPending => (true, Some(STALE_PENDING_TIMEOUT_SECONDS)),
            WorkoutSummaryError::AlreadyExists
            | WorkoutSummaryError::Locked
            | WorkoutSummaryError::NotFound
            | WorkoutSummaryError::Validation(_) => (false, None),
        };

        TaskRunOutcome::Failed {
            checkpoint: serde_json::to_value(serialize_workout_summary_error(&error)).ok(),
            error_message: error.to_string(),
            retryable,
            retry_delay_seconds,
        }
    }

    fn build_completed_checkpoint(task_id: &str, reply: &CoachReply) -> Option<serde_json::Value> {
        match serialize_completed_coach_reply_checkpoint(reply) {
            Ok(value) => Some(value),
            Err(error) => {
                warn!(task_id = %task_id, %error, "failed to serialize completed coach reply checkpoint");
                None
            }
        }
    }
}

impl<Base> ScheduledTaskExecutor for WorkoutSummaryCoachReplyTaskExecutor<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    type Payload = WorkoutSummaryCoachReplyTaskPayload;
    type Output = CoachReply;
    type Error = WorkoutSummaryError;

    fn task_type(&self) -> &'static str {
        COACH_REPLY_TASK_TYPE
    }

    fn parse_error(&self, error: serde_json::Error) -> Self::Error {
        WorkoutSummaryError::Repository(format!(
            "invalid workout summary coach reply task payload: {error}"
        ))
    }

    fn on_parse_error(&self, task: &ScheduledTask, error: &Self::Error) {
        warn!(task_id = %task.id, %error, "invalid workout summary coach reply task payload");
    }

    fn run(
        &self,
        _task: ScheduledTask,
        payload: Self::Payload,
    ) -> BoxFuture<Result<Self::Output, Self::Error>> {
        let base = self.base.clone();
        Box::pin(async move {
            base.generate_coach_reply(
                &payload.user_id,
                &payload.workout_id,
                payload.user_message_id,
            )
            .await
        })
    }

    fn completed_checkpoint(
        &self,
        task_id: &str,
        output: &Self::Output,
    ) -> Result<Option<serde_json::Value>, Self::Error> {
        Ok(Self::build_completed_checkpoint(task_id, output))
    }

    fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome {
        Self::map_task_failure(error)
    }
}

pub fn workout_summary_coach_reply_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: WorkoutSummaryUseCases + 'static,
{
    Arc::new(scheduled_task_handler(
        WorkoutSummaryCoachReplyTaskExecutor { base },
    ))
}
