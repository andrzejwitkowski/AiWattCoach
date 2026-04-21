use super::*;

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub fn sweep_timed_out_tasks(
        &self,
        worker_stale_after_seconds: i64,
        limit: usize,
    ) -> BoxFuture<Result<usize, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let workers = self.workers.clone();
        let scheduler = self.clone();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let candidates = tasks
                .list_timeout_candidates(now_epoch_seconds, limit)
                .await?;
            let mut timed_out_count = 0usize;

            for task in candidates {
                if let Some(updated_task) = sweep_timeout_candidate(
                    &tasks,
                    &workers,
                    task,
                    now_epoch_seconds,
                    worker_stale_after_seconds,
                )
                .await?
                {
                    scheduler.publish_task_update(updated_task.clone()).await;
                    if task_is_terminal(&updated_task) {
                        scheduler
                            .publish_terminal_task_update(Some(updated_task))
                            .await;
                    }
                    timed_out_count += 1;
                }
            }

            Ok(timed_out_count)
        })
    }

    pub fn retry_task(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        let request = TaskRetryRequest {
            task_id: task_id.to_string(),
            retried_at_epoch_seconds: self.clock.now_epoch_seconds(),
        };
        Box::pin(async move {
            let task = tasks.retry(request).await?;
            if let Some(task) = task.clone() {
                scheduler.publish_task_update(task).await;
            }
            Ok(task)
        })
    }
}

async fn sweep_timeout_candidate<Tasks, Workers>(
    tasks: &Tasks,
    workers: &Workers,
    task: ScheduledTask,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
{
    let Some(worker_id) = task.claimed_by.clone() else {
        if task_heartbeat_is_fresh(&task, now_epoch_seconds, worker_stale_after_seconds) {
            return Ok(None);
        }

        return recover_task(tasks, &task, None, now_epoch_seconds).await;
    };

    let worker = workers.find_by_worker_id(&worker_id).await?;
    let worker_is_fresh = worker_is_fresh(
        worker.as_ref(),
        now_epoch_seconds,
        worker_stale_after_seconds,
    );
    let worker_reports_task_active = worker_reports_task_active(worker.as_ref(), &task.id);

    if worker_is_fresh && worker_reports_task_active {
        return Ok(None);
    }

    if worker_is_fresh && !worker_reports_task_active {
        return recover_task(tasks, &task, Some(worker_id), now_epoch_seconds).await;
    }

    if task_heartbeat_is_fresh(&task, now_epoch_seconds, worker_stale_after_seconds) {
        return Ok(None);
    }

    if worker.is_none() {
        return recover_task(tasks, &task, Some(worker_id), now_epoch_seconds).await;
    }

    mark_task_timed_out(tasks, &task, worker_id, now_epoch_seconds).await
}

fn task_heartbeat_is_fresh(
    task: &ScheduledTask,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> bool {
    task.last_heartbeat_at_epoch_seconds
        .is_some_and(|last_heartbeat_at_epoch_seconds| {
            last_heartbeat_at_epoch_seconds >= now_epoch_seconds - worker_stale_after_seconds
        })
}

fn worker_is_fresh(
    worker: Option<&TaskWorker>,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> bool {
    worker.is_some_and(|worker| {
        worker.last_heartbeat_at_epoch_seconds >= now_epoch_seconds - worker_stale_after_seconds
    })
}

fn worker_reports_task_active(worker: Option<&TaskWorker>, task_id: &str) -> bool {
    worker.is_some_and(|worker| {
        worker
            .active_task_ids
            .iter()
            .any(|active| active == task_id)
    })
}

async fn recover_task<Tasks>(
    tasks: &Tasks,
    task: &ScheduledTask,
    expected_claimed_by: Option<String>,
    recovered_at_epoch_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
{
    let changed = tasks
        .recover(TaskRecoverRequest {
            task_id: task.id.clone(),
            expected_claimed_by,
            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
            recovered_at_epoch_seconds,
        })
        .await?;

    if !changed {
        return Ok(None);
    }

    tasks.find_by_id(&task.id).await
}

async fn mark_task_timed_out<Tasks>(
    tasks: &Tasks,
    task: &ScheduledTask,
    worker_id: String,
    timed_out_at_epoch_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
{
    let changed = tasks
        .mark_timed_out(TaskMarkTimedOutRequest {
            task_id: task.id.clone(),
            expected_claimed_by: Some(worker_id),
            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
            timed_out_at_epoch_seconds,
        })
        .await?;

    if !changed {
        return Ok(None);
    }

    tasks.find_by_id(&task.id).await
}
