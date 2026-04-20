use tracing::warn;

use super::checkpoint::serialize_completed_coach_reply_checkpoint;
use super::*;

fn llm_error_is_retryable(error: &LlmError) -> bool {
    error.is_retryable()
}

struct WorkoutSummaryCoachReplyTaskHandler<Base> {
    base: Arc<Base>,
}

impl<Base> WorkoutSummaryCoachReplyTaskHandler<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    fn map_task_failure(error: WorkoutSummaryError) -> TaskRunOutcome {
        let retryable = match &error {
            WorkoutSummaryError::Llm(llm_error) => llm_error_is_retryable(llm_error),
            WorkoutSummaryError::Repository(_) => true,
            WorkoutSummaryError::ReplyAlreadyPending => true,
            WorkoutSummaryError::AlreadyExists
            | WorkoutSummaryError::Locked
            | WorkoutSummaryError::NotFound
            | WorkoutSummaryError::Validation(_) => false,
        };

        TaskRunOutcome::Failed {
            checkpoint: serde_json::to_value(serialize_workout_summary_error(&error)).ok(),
            error_message: error.to_string(),
            retryable,
        }
    }

    fn completed_checkpoint(task_id: &str, reply: &CoachReply) -> Option<serde_json::Value> {
        match serialize_completed_coach_reply_checkpoint(reply) {
            Ok(value) => Some(value),
            Err(error) => {
                warn!(task_id = %task_id, %error, "failed to serialize completed coach reply checkpoint");
                None
            }
        }
    }
}

impl<Base> TaskHandler for WorkoutSummaryCoachReplyTaskHandler<Base>
where
    Base: WorkoutSummaryUseCases + 'static,
{
    fn task_type(&self) -> &'static str {
        COACH_REPLY_TASK_TYPE
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let base = self.base.clone();
        Box::pin(async move {
            let payload = match parse_task_payload(&task) {
                Ok(payload) => payload,
                Err(error) => {
                    warn!(task_id = %task.id, %error, "invalid workout summary coach reply task payload");
                    return TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: error.to_string(),
                        retryable: false,
                    };
                }
            };

            match base
                .generate_coach_reply(
                    &payload.user_id,
                    &payload.workout_id,
                    payload.user_message_id,
                )
                .await
            {
                Ok(reply) => TaskRunOutcome::Completed {
                    checkpoint: Self::completed_checkpoint(&task.id, &reply),
                },
                Err(error) => Self::map_task_failure(error),
            }
        })
    }
}

pub fn workout_summary_coach_reply_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: WorkoutSummaryUseCases + 'static,
{
    Arc::new(WorkoutSummaryCoachReplyTaskHandler { base })
}

pub fn spawn_workout_summary_coach_reply_task_runner<Base, Tasks, Workers, Time>(
    base: Arc<Base>,
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
) -> tokio::task::JoinHandle<()>
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
