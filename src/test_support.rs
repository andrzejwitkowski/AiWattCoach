use std::sync::Arc;

use crate::{
    config::{spawn_task_worker, workout_summary_task_worker_config},
    domain::{
        identity::Clock,
        task_scheduler::{
            TaskRepository, TaskSchedulerError, TaskSchedulerService, TaskWorkerRepository,
        },
        workout_summary::{workout_summary_coach_reply_task_handler, WorkoutSummaryUseCases},
    },
    BackgroundTaskHandle,
};

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
        workout_summary_task_worker_config(),
        vec![workout_summary_coach_reply_task_handler(base)],
    )
}
