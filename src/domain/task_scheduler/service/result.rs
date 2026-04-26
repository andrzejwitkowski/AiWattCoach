use super::*;

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub fn enqueue_result_task<Handler>(
        &self,
        task: ScheduledTask,
        map_scheduler_error: fn(TaskSchedulerError) -> Handler::Error,
        handler: Handler,
    ) -> BoxFuture<Result<Handler::Output, Handler::Error>>
    where
        Handler: ResultTaskHandler,
    {
        let scheduler = self.clone();
        Box::pin(async move {
            let task = scheduler
                .enqueue(task)
                .await
                .map(|result| result.task)
                .map_err(map_scheduler_error)?;
            let task = scheduler
                .retry_if_terminal(task)
                .await
                .map_err(map_scheduler_error)?;
            scheduler
                .wait_for_result_task(&task.id, map_scheduler_error, handler)
                .await
        })
    }

    pub fn wait_for_result_task<Handler>(
        &self,
        task_id: &str,
        map_scheduler_error: fn(TaskSchedulerError) -> Handler::Error,
        handler: Handler,
    ) -> BoxFuture<Result<Handler::Output, Handler::Error>>
    where
        Handler: ResultTaskHandler,
    {
        let scheduler = self.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let mut watcher = scheduler.subscribe_to_task_updates(&task_id).await;
            let mut current = scheduler
                .get_task(&task_id)
                .await
                .map_err(map_scheduler_error)?;

            loop {
                match current {
                    Some(task) => match task.status {
                        TaskStatus::Completed => {
                            let completed = handler.parse_completed(&task)?;
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return handler.finish(completed).await;
                        }
                        TaskStatus::Failed => {
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return Err(handler.parse_failed(&task)?);
                        }
                        TaskStatus::TimedOut => {
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return Err(handler.task_timed_out(&task_id));
                        }
                        _ => {}
                    },
                    None => {
                        scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                        return Err(handler.task_disappeared(&task_id));
                    }
                }

                watcher
                    .changed()
                    .await
                    .map_err(|_| handler.task_disappeared(&task_id))?;
                current = watcher.borrow().clone();
            }
        })
    }

    pub(super) async fn retry_if_terminal(
        &self,
        task: ScheduledTask,
    ) -> Result<ScheduledTask, TaskSchedulerError> {
        if !task.can_retry_manually() {
            return Ok(task);
        }

        Ok(self.retry_task(&task.id).await?.unwrap_or(task))
    }
}
