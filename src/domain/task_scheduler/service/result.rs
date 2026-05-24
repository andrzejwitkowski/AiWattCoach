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
                            drop(watcher);
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return handler.finish(completed).await;
                        }
                        TaskStatus::Failed => {
                            drop(watcher);
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return Err(handler.parse_failed(&task)?);
                        }
                        TaskStatus::TimedOut => {
                            drop(watcher);
                            scheduler.cleanup_task_waiter_if_unused(&task_id).await;
                            return Err(handler.task_timed_out(&task_id));
                        }
                        _ => {}
                    },
                    None => {
                        drop(watcher);
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::domain::{identity::Clock, task_scheduler::*};

    #[derive(Clone)]
    struct TestClock {
        now_epoch_seconds: i64,
    }

    impl TestClock {
        fn new(now_epoch_seconds: i64) -> Self {
            Self { now_epoch_seconds }
        }
    }

    impl Clock for TestClock {
        fn now_epoch_seconds(&self) -> i64 {
            self.now_epoch_seconds
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryTaskRepository {
        tasks: Arc<Mutex<HashMap<String, ScheduledTask>>>,
    }

    impl TaskRepository for InMemoryTaskRepository {
        fn enqueue_if_absent(
            &self,
            task: ScheduledTask,
        ) -> BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            Box::pin(async move {
                let mut tasks = tasks.lock().await;
                if let Some(existing) = tasks.values().find(|existing| {
                    existing.user_id == task.user_id && existing.dedupe_key == task.dedupe_key
                }) {
                    return Ok(TaskEnqueueResult {
                        task: existing.clone(),
                        created: false,
                    });
                }

                tasks.insert(task.id.clone(), task.clone());
                Ok(TaskEnqueueResult {
                    task,
                    created: true,
                })
            })
        }

        fn claim_next_due(
            &self,
            _request: TaskClaimRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn heartbeat(
            &self,
            _request: TaskHeartbeatRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn save_checkpoint(
            &self,
            _request: TaskCheckpointRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn complete(
            &self,
            request: TaskCompleteRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            Box::pin(async move {
                let mut tasks = tasks.lock().await;
                let Some(task) = tasks.get_mut(&request.task_id) else {
                    return Ok(None);
                };

                task.status = TaskStatus::Completed;
                task.checkpoint = request.checkpoint;
                task.error_message = None;
                task.updated_at_epoch_seconds = request.completed_at_epoch_seconds;
                task.finished_at_epoch_seconds = Some(request.completed_at_epoch_seconds);
                Ok(Some(task.clone()))
            })
        }

        fn fail(
            &self,
            request: TaskFailRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            Box::pin(async move {
                let mut tasks = tasks.lock().await;
                let Some(task) = tasks.get_mut(&request.task_id) else {
                    return Ok(None);
                };

                task.status = if request.retry_at_epoch_seconds.is_some() {
                    TaskStatus::RetryScheduled
                } else {
                    TaskStatus::Failed
                };
                task.checkpoint = request.checkpoint;
                task.error_message = Some(request.error_message);
                task.next_attempt_at_epoch_seconds = request
                    .retry_at_epoch_seconds
                    .unwrap_or(request.failed_at_epoch_seconds);
                task.updated_at_epoch_seconds = request.failed_at_epoch_seconds;
                task.finished_at_epoch_seconds = request
                    .retry_at_epoch_seconds
                    .map(|_| None)
                    .unwrap_or(Some(request.failed_at_epoch_seconds));
                Ok(Some(task.clone()))
            })
        }

        fn list_timeout_candidates(
            &self,
            _now_epoch_seconds: i64,
            _limit: usize,
        ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn mark_timed_out(
            &self,
            _request: TaskMarkTimedOutRequest,
        ) -> BoxFuture<Result<bool, TaskSchedulerError>> {
            Box::pin(async { Ok(false) })
        }

        fn recover(
            &self,
            _request: TaskRecoverRequest,
        ) -> BoxFuture<Result<bool, TaskSchedulerError>> {
            Box::pin(async { Ok(false) })
        }

        fn retry(
            &self,
            _request: TaskRetryRequest,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn find_by_id(
            &self,
            task_id: &str,
        ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            let task_id = task_id.to_string();
            Box::pin(async move { Ok(tasks.lock().await.get(&task_id).cloned()) })
        }

        fn list(
            &self,
            _filter: TaskListFilter,
        ) -> BoxFuture<Result<TaskListPage, TaskSchedulerError>> {
            Box::pin(async {
                Ok(TaskListPage {
                    tasks: Vec::new(),
                    has_next_page: false,
                })
            })
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryTaskWorkerRepository;

    impl TaskWorkerRepository for InMemoryTaskWorkerRepository {
        fn upsert(&self, worker: TaskWorker) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
            Box::pin(async move { Ok(worker) })
        }

        fn touch_heartbeat(
            &self,
            worker_id: &str,
            is_leader: bool,
            enabled_task_types: Vec<String>,
            last_heartbeat_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
            let worker = TaskWorker {
                worker_id: worker_id.to_string(),
                is_leader,
                enabled_task_types,
                active_task_ids: Vec::new(),
                last_heartbeat_at_epoch_seconds,
            };
            Box::pin(async move { Ok(worker) })
        }

        fn find_by_worker_id(
            &self,
            _worker_id: &str,
        ) -> BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }
    }

    struct StubResultHandler;

    impl ResultTaskHandler for StubResultHandler {
        type Completed = String;
        type Output = String;
        type Error = String;

        fn task_disappeared(&self, task_id: &str) -> Self::Error {
            format!("task disappeared: {task_id}")
        }

        fn task_timed_out(&self, task_id: &str) -> Self::Error {
            format!("task timed out: {task_id}")
        }

        fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error> {
            task.checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.get("value"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| "missing completed checkpoint".to_string())
        }

        fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error> {
            Ok(task
                .error_message
                .clone()
                .unwrap_or_else(|| "missing failed message".to_string()))
        }

        fn finish(
            &self,
            completed: Self::Completed,
        ) -> BoxFuture<Result<Self::Output, Self::Error>> {
            Box::pin(async move { Ok(completed) })
        }
    }

    fn scheduler(
    ) -> TaskSchedulerService<InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock> {
        TaskSchedulerService::new(
            InMemoryTaskRepository::default(),
            InMemoryTaskWorkerRepository,
            TestClock::new(1),
        )
    }

    fn queued_task(task_id: &str) -> ScheduledTask {
        ScheduledTask::new(
            NewTask {
                id: task_id.to_string(),
                user_id: "user-1".to_string(),
                task_type: "test.task".to_string(),
                payload: json!({}),
                retry_strategy: RetryStrategy::Never,
                dedupe_key: format!("dedupe:{task_id}"),
                execution_timeout_seconds: 30,
                leader_only: false,
            },
            1,
        )
        .expect("task should build")
    }

    #[tokio::test]
    async fn wait_for_result_task_cleans_up_waiter_for_immediately_completed_task() {
        let scheduler = scheduler();
        let tasks = scheduler.tasks.clone();
        let task_id = "task-completed";

        let queued = queued_task(task_id);
        tasks
            .enqueue_if_absent(queued)
            .await
            .expect("task should enqueue");
        scheduler
            .complete_task(task_id, "worker-1", Some(json!({ "value": "done" })))
            .await
            .expect("task should complete")
            .expect("completed task should exist");

        let result = scheduler
            .wait_for_result_task(
                task_id,
                |error: TaskSchedulerError| error.to_string(),
                StubResultHandler,
            )
            .await
            .expect("completed task should replay");

        assert_eq!(result, "done");
        assert_eq!(scheduler.test_waiter_count().await, 0);
    }

    #[tokio::test]
    async fn wait_for_result_task_cleans_up_waiter_for_immediately_failed_task() {
        let scheduler = scheduler();
        let tasks = scheduler.tasks.clone();
        let task_id = "task-failed";

        let queued = queued_task(task_id);
        let queued = tasks
            .enqueue_if_absent(queued)
            .await
            .expect("task should enqueue")
            .task;
        scheduler
            .fail_task(FailTaskInput {
                task_id,
                worker_id: "worker-1",
                checkpoint: None,
                error_message: "failed immediately".to_string(),
                retryable: false,
                retry_delay_seconds: None,
                retry_strategy: &queued.retry_strategy,
                attempt_count: queued.attempt_count.saturating_add(1),
            })
            .await
            .expect("task should fail")
            .expect("failed task should exist");

        let error = scheduler
            .wait_for_result_task(
                task_id,
                |error: TaskSchedulerError| error.to_string(),
                StubResultHandler,
            )
            .await
            .expect_err("failed task should replay error");

        assert_eq!(error, "failed immediately");
        assert_eq!(scheduler.test_waiter_count().await, 0);
    }
}
