use std::sync::Arc;
use std::time::Duration;

use crate::{
    config::spawn_task_worker,
    domain::{
        identity::Clock,
        task_scheduler::{
            TaskRepository, TaskSchedulerError, TaskSchedulerService, TaskWorkerConfig,
            TaskWorkerRepository,
        },
        workout_summary::{
            workout_summary_coach_reply_task_handler, WorkoutSummaryUseCases,
            COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS, COACH_REPLY_LEASE_DURATION_SECONDS,
            COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS,
        },
    },
    BackgroundTaskHandle,
};

fn test_workout_summary_task_worker_config() -> TaskWorkerConfig {
    TaskWorkerConfig {
        is_leader: false,
        lease_duration_seconds: COACH_REPLY_LEASE_DURATION_SECONDS,
        heartbeat_interval: Duration::from_secs(COACH_REPLY_HEARTBEAT_INTERVAL_SECONDS),
        idle_poll_interval: Duration::from_millis(COACH_REPLY_WAIT_POLL_INTERVAL_MILLIS),
        max_concurrency: 4,
    }
}

pub(crate) fn spawn_test_workout_summary_task_worker<Base, Tasks, Workers, Time>(
    base: Arc<Base>,
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
) -> Result<BackgroundTaskHandle, TaskSchedulerError>
where
    Base: WorkoutSummaryUseCases + 'static,
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    spawn_task_worker(
        scheduler,
        worker_id,
        test_workout_summary_task_worker_config(),
        vec![workout_summary_coach_reply_task_handler(base)],
    )
}
