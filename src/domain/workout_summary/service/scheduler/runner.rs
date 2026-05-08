use crate::domain::task_scheduler::{
    scheduled_task_handler, BoxFuture as TaskSchedulerBoxFuture, ScheduledTaskRunner,
    SharedTaskHandler, TaskFailurePolicy,
};
use std::sync::Arc;
use std::time::Duration;

use super::super::STALE_PENDING_TIMEOUT_SECONDS;
use super::checkpoint::serialize_completed_coach_reply_checkpoint;
use super::*;

fn llm_error_is_retryable(error: &LlmError) -> bool {
    error.is_retryable()
}

struct WorkoutSummaryCoachReplyRunner<Base> {
    base: Arc<Base>,
}

impl<Base> ScheduledTaskRunner for WorkoutSummaryCoachReplyRunner<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    type Payload = WorkoutSummaryCoachReplyTaskPayload;
    type Output = CoachReply;
    type Error = WorkoutSummaryError;

    fn task_type(&self) -> &'static str {
        COACH_REPLY_TASK_TYPE
    }

    fn execute(
        &self,
        payload: Self::Payload,
    ) -> TaskSchedulerBoxFuture<Result<Self::Output, Self::Error>> {
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

    fn serialize_checkpoint(
        &self,
        output: &Self::Output,
    ) -> Result<serde_json::Value, Self::Error> {
        serialize_completed_coach_reply_checkpoint(output)
    }

    fn serialize_error(&self, error: &Self::Error) -> Option<serde_json::Value> {
        serde_json::to_value(serialize_workout_summary_error(error)).ok()
    }

    fn failure_policy(&self, error: &Self::Error) -> TaskFailurePolicy {
        let (retryable, retry_delay_seconds) = match error {
            WorkoutSummaryError::Llm(llm_error) => (llm_error_is_retryable(llm_error), None),
            WorkoutSummaryError::Repository(_) => (true, None),
            WorkoutSummaryError::ReplyAlreadyPending => (true, Some(STALE_PENDING_TIMEOUT_SECONDS)),
            WorkoutSummaryError::AlreadyExists
            | WorkoutSummaryError::Locked
            | WorkoutSummaryError::NotFound
            | WorkoutSummaryError::Validation(_) => (false, None),
        };
        TaskFailurePolicy {
            retryable,
            retry_delay_seconds,
        }
    }
}

pub fn workout_summary_coach_reply_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: WorkoutSummaryUseCases + 'static,
{
    scheduled_task_handler(Arc::new(WorkoutSummaryCoachReplyRunner { base }))
}

pub fn spawn_workout_summary_coach_reply_task_runner<Base, Tasks, Workers, Time>(
    base: Arc<Base>,
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
) -> Result<crate::BackgroundTaskHandle, TaskSchedulerError>
where
    Base: WorkoutSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    spawn_task_worker(
        scheduler,
        worker_id,
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: COACH_REPLY_LEASE_DURATION_SECONDS,
            heartbeat_interval: Duration::from_secs(COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS),
            idle_poll_interval: Duration::from_millis(COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS),
            max_concurrency: 4,
        },
        vec![workout_summary_coach_reply_task_handler(base)],
    )
}
