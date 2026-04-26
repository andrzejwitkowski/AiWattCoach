use std::sync::Arc;

use crate::domain::{
    identity::{Clock, IdGenerator},
    task_scheduler::{
        BoxFuture, NewTask, RetryStrategy, ScheduledTask, SharedTaskHandler, TaskHandler,
        TaskRepository, TaskRunOutcome, TaskSchedulerError, TaskSchedulerService,
        TaskWorkerRepository,
    },
    wahoo::WahooError,
    wahoo_fit_files::WahooFitFileRepository,
};

use super::{WahooFitEnrichmentError, WahooFitEnrichmentTaskPayload};

pub const WAHOO_FIT_ENRICHMENT_TASK_TYPE: &str = "wahoo_fit.enrich";
const WAHOO_FIT_ENRICHMENT_RETRY_MAX_ATTEMPTS: u32 = 3;
const WAHOO_FIT_ENRICHMENT_RETRY_DELAY_SECONDS: i64 = 5 * 60;
const WAHOO_FIT_ENRICHMENT_EXECUTION_TIMEOUT_SECONDS: i64 = 3 * 60;

pub trait WahooFitEnrichmentExecutionUseCases: Send + Sync + 'static {
    fn enrich_completed_workout(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> BoxFuture<Result<(), WahooFitEnrichmentError>>;
}

pub trait WahooFitEnrichmentQueueUseCases: Send + Sync + 'static {
    fn enqueue_enrichment(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> BoxFuture<Result<(), WahooFitEnrichmentError>>;
}

#[derive(Clone)]
pub struct SchedulerBackedWahooFitEnrichmentService<Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    ids: Ids,
}

impl<Tasks, Workers, Time, Ids> SchedulerBackedWahooFitEnrichmentService<Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(scheduler: TaskSchedulerService<Tasks, Workers, Time>, ids: Ids) -> Self {
        Self { scheduler, ids }
    }

    fn build_enrichment_task(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> Result<ScheduledTask, WahooFitEnrichmentError> {
        ScheduledTask::new(
            NewTask {
                id: self.ids.new_id("task"),
                user_id: user_id.to_string(),
                task_type: WAHOO_FIT_ENRICHMENT_TASK_TYPE.to_string(),
                payload: serde_json::to_value(WahooFitEnrichmentTaskPayload {
                    completed_workout_id: completed_workout_id.to_string(),
                    wahoo_workout_id,
                })
                .map_err(|error| {
                    WahooFitEnrichmentError::Scheduler(format!(
                        "failed to serialize Wahoo FIT enrichment task payload: {error}"
                    ))
                })?,
                retry_strategy: RetryStrategy::Fixed {
                    max_attempts: WAHOO_FIT_ENRICHMENT_RETRY_MAX_ATTEMPTS,
                    delay_seconds: WAHOO_FIT_ENRICHMENT_RETRY_DELAY_SECONDS,
                },
                dedupe_key: wahoo_fit_enrichment_dedupe_key(completed_workout_id),
                execution_timeout_seconds: WAHOO_FIT_ENRICHMENT_EXECUTION_TIMEOUT_SECONDS,
                leader_only: false,
            },
            self.scheduler.now_epoch_seconds(),
        )
        .map_err(map_task_scheduler_error)
    }
}

impl<Tasks, Workers, Time, Ids> WahooFitEnrichmentQueueUseCases
    for SchedulerBackedWahooFitEnrichmentService<Tasks, Workers, Time, Ids>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    fn enqueue_enrichment(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> BoxFuture<Result<(), WahooFitEnrichmentError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let task =
                service.build_enrichment_task(&user_id, &completed_workout_id, wahoo_workout_id)?;
            service
                .scheduler
                .enqueue_no_result_task(task)
                .await
                .map_err(map_task_scheduler_error)?;
            Ok(())
        })
    }
}

struct WahooFitEnrichmentTaskHandler<Base> {
    base: Arc<Base>,
}

impl<Base> TaskHandler for WahooFitEnrichmentTaskHandler<Base>
where
    Base: WahooFitEnrichmentExecutionUseCases,
{
    fn task_type(&self) -> &'static str {
        WAHOO_FIT_ENRICHMENT_TASK_TYPE
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let base = self.base.clone();
        Box::pin(async move {
            let payload = match parse_task_payload(&task) {
                Ok(payload) => payload,
                Err(error) => {
                    return TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: error.to_string(),
                        retryable: false,
                        retry_delay_seconds: None,
                    };
                }
            };

            match base
                .enrich_completed_workout(
                    &task.user_id,
                    &payload.completed_workout_id,
                    payload.wahoo_workout_id,
                )
                .await
            {
                Ok(()) => TaskRunOutcome::Completed { checkpoint: None },
                Err(error) => map_task_failure(error),
            }
        })
    }
}

pub fn wahoo_fit_enrichment_task_handler<Base>(base: Arc<Base>) -> SharedTaskHandler
where
    Base: WahooFitEnrichmentExecutionUseCases,
{
    Arc::new(WahooFitEnrichmentTaskHandler { base })
}

fn parse_task_payload(
    task: &ScheduledTask,
) -> Result<WahooFitEnrichmentTaskPayload, WahooFitEnrichmentError> {
    serde_json::from_value(task.payload.clone()).map_err(|error| {
        WahooFitEnrichmentError::Scheduler(format!(
            "invalid Wahoo FIT enrichment task payload: {error}"
        ))
    })
}

fn map_task_failure(error: WahooFitEnrichmentError) -> TaskRunOutcome {
    TaskRunOutcome::Failed {
        checkpoint: None,
        error_message: error.to_string(),
        retryable: error_is_retryable(&error),
        retry_delay_seconds: None,
    }
}

fn error_is_retryable(error: &WahooFitEnrichmentError) -> bool {
    match error {
        WahooFitEnrichmentError::NotFound
        | WahooFitEnrichmentError::DownloadUnavailable(_)
        | WahooFitEnrichmentError::Parse(_) => false,
        WahooFitEnrichmentError::CompletedWorkoutRepository(_)
        | WahooFitEnrichmentError::FitFileRepository(_)
        | WahooFitEnrichmentError::Scheduler(_)
        | WahooFitEnrichmentError::TrainingLoad(_) => true,
        WahooFitEnrichmentError::Wahoo(error) => {
            matches!(error, WahooError::External(_) | WahooError::Repository(_))
        }
    }
}

fn wahoo_fit_enrichment_dedupe_key(completed_workout_id: &str) -> String {
    format!("wahoo-fit:{completed_workout_id}")
}

fn map_task_scheduler_error(error: TaskSchedulerError) -> WahooFitEnrichmentError {
    WahooFitEnrichmentError::Scheduler(error.to_string())
}

impl<Wahoo, Workouts, FitFiles, Parser, Time> WahooFitEnrichmentExecutionUseCases
    for super::WahooFitEnrichmentService<Wahoo, Workouts, FitFiles, Parser, Time>
where
    Wahoo: crate::domain::wahoo::WahooUseCases + ?Sized + 'static,
    Workouts: crate::domain::completed_workouts::CompletedWorkoutRepository,
    FitFiles: WahooFitFileRepository,
    Parser: super::WahooFitParserPort,
    Time: Clock + 'static,
{
    fn enrich_completed_workout(
        &self,
        user_id: &str,
        completed_workout_id: &str,
        wahoo_workout_id: i64,
    ) -> BoxFuture<Result<(), WahooFitEnrichmentError>> {
        super::WahooFitEnrichmentService::enrich_completed_workout(
            self,
            user_id,
            completed_workout_id,
            wahoo_workout_id,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        identity::{Clock, IdGenerator},
        task_scheduler::{
            BoxFuture as TaskBoxFuture, ScheduledTask, TaskClaimRequest, TaskCompleteRequest,
            TaskEnqueueResult, TaskFailRequest, TaskHeartbeatRequest, TaskListFilter,
            TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
            TaskSchedulerService, TaskStatus, TaskWorker, TaskWorkerRepository,
        },
    };

    use super::*;

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            1_700_000_000
        }
    }

    #[derive(Clone)]
    struct FixedIds;

    impl IdGenerator for FixedIds {
        fn new_id(&self, prefix: &str) -> String {
            format!("{prefix}-1")
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryTaskRepository {
        tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    }

    impl InMemoryTaskRepository {
        fn only_task(&self) -> ScheduledTask {
            self.tasks.lock().unwrap()[0].clone()
        }

        fn mark_failed(&self, task_id: &str, now_epoch_seconds: i64) {
            let mut tasks = self.tasks.lock().unwrap();
            let task = tasks.iter_mut().find(|task| task.id == task_id).unwrap();
            task.status = TaskStatus::Failed;
            task.finished_at_epoch_seconds = Some(now_epoch_seconds);
        }
    }

    impl TaskRepository for InMemoryTaskRepository {
        fn enqueue_if_absent(
            &self,
            task: ScheduledTask,
        ) -> TaskBoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            Box::pin(async move {
                let mut tasks = tasks.lock().unwrap();
                if let Some(existing) = tasks.iter().find(|existing| {
                    existing.user_id == task.user_id && existing.dedupe_key == task.dedupe_key
                }) {
                    return Ok(TaskEnqueueResult {
                        task: existing.clone(),
                        created: false,
                    });
                }
                tasks.push(task.clone());
                Ok(TaskEnqueueResult {
                    task,
                    created: true,
                })
            })
        }

        fn claim_next_due(
            &self,
            _request: TaskClaimRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn heartbeat(
            &self,
            _request: TaskHeartbeatRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn save_checkpoint(
            &self,
            _request: crate::domain::task_scheduler::TaskCheckpointRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn complete(
            &self,
            _request: TaskCompleteRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn fail(
            &self,
            _request: TaskFailRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }

        fn list_timeout_candidates(
            &self,
            _now_epoch_seconds: i64,
            _limit: usize,
        ) -> TaskBoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn mark_timed_out(
            &self,
            _request: TaskMarkTimedOutRequest,
        ) -> TaskBoxFuture<Result<bool, TaskSchedulerError>> {
            Box::pin(async { Ok(false) })
        }

        fn recover(
            &self,
            _request: TaskRecoverRequest,
        ) -> TaskBoxFuture<Result<bool, TaskSchedulerError>> {
            Box::pin(async { Ok(false) })
        }

        fn retry(
            &self,
            request: TaskRetryRequest,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            Box::pin(async move {
                let mut tasks = tasks.lock().unwrap();
                let Some(task) = tasks.iter_mut().find(|task| task.id == request.task_id) else {
                    return Ok(None);
                };
                if !task.can_retry_manually() {
                    return Ok(None);
                }
                task.status = TaskStatus::Queued;
                task.finished_at_epoch_seconds = None;
                task.next_attempt_at_epoch_seconds = request.retried_at_epoch_seconds;
                Ok(Some(task.clone()))
            })
        }

        fn find_by_id(
            &self,
            task_id: &str,
        ) -> TaskBoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
            let tasks = self.tasks.clone();
            let task_id = task_id.to_string();
            Box::pin(async move {
                Ok(tasks
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|task| task.id == task_id)
                    .cloned())
            })
        }

        fn list(
            &self,
            _filter: TaskListFilter,
        ) -> TaskBoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryTaskWorkerRepository;

    impl TaskWorkerRepository for InMemoryTaskWorkerRepository {
        fn upsert(
            &self,
            worker: TaskWorker,
        ) -> TaskBoxFuture<Result<TaskWorker, TaskSchedulerError>> {
            Box::pin(async move { Ok(worker) })
        }

        fn touch_heartbeat(
            &self,
            worker_id: &str,
            is_leader: bool,
            enabled_task_types: Vec<String>,
            last_heartbeat_at_epoch_seconds: i64,
        ) -> TaskBoxFuture<Result<TaskWorker, TaskSchedulerError>> {
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
        ) -> TaskBoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>> {
            Box::pin(async { Ok(None) })
        }
    }

    #[tokio::test]
    async fn enqueue_uses_completed_workout_dedupe_key_and_retries_failed_task() {
        let tasks = InMemoryTaskRepository::default();
        let scheduler =
            TaskSchedulerService::new(tasks.clone(), InMemoryTaskWorkerRepository, FixedClock);
        let service = SchedulerBackedWahooFitEnrichmentService::new(scheduler, FixedIds);

        service
            .enqueue_enrichment("user-1", "wahoo-workout:42", 42)
            .await
            .expect("first enqueue should succeed");

        let task = tasks.only_task();
        assert_eq!(task.dedupe_key, "wahoo-fit:wahoo-workout:42");

        tasks.mark_failed(&task.id, 1_700_000_000);

        service
            .enqueue_enrichment("user-1", "wahoo-workout:42", 42)
            .await
            .expect("re-enqueue should revive failed task");

        assert_eq!(tasks.only_task().status, TaskStatus::Queued);
    }

    #[tokio::test]
    async fn task_handler_uses_task_user_id_instead_of_payload_user_id() {
        #[derive(Clone, Default)]
        struct RecordingExecutor {
            calls: Arc<Mutex<Vec<(String, String, i64)>>>,
        }

        impl WahooFitEnrichmentExecutionUseCases for RecordingExecutor {
            fn enrich_completed_workout(
                &self,
                user_id: &str,
                completed_workout_id: &str,
                wahoo_workout_id: i64,
            ) -> BoxFuture<Result<(), WahooFitEnrichmentError>> {
                let calls = self.calls.clone();
                let user_id = user_id.to_string();
                let completed_workout_id = completed_workout_id.to_string();
                Box::pin(async move {
                    calls
                        .lock()
                        .unwrap()
                        .push((user_id, completed_workout_id, wahoo_workout_id));
                    Ok(())
                })
            }
        }

        let executor = Arc::new(RecordingExecutor::default());
        let handler = wahoo_fit_enrichment_task_handler(executor.clone());
        let task = ScheduledTask::new(
            NewTask {
                id: "task-1".to_string(),
                user_id: "task-user".to_string(),
                task_type: WAHOO_FIT_ENRICHMENT_TASK_TYPE.to_string(),
                payload: serde_json::json!({
                    "user_id": "payload-user",
                    "completed_workout_id": "wahoo-workout:42",
                    "wahoo_workout_id": 42,
                }),
                retry_strategy: RetryStrategy::Fixed {
                    max_attempts: 3,
                    delay_seconds: 300,
                },
                dedupe_key: "wahoo-fit:wahoo-workout:42".to_string(),
                execution_timeout_seconds: 180,
                leader_only: false,
            },
            1_700_000_000,
        )
        .unwrap();

        let outcome = handler.run(task).await;

        assert!(matches!(
            outcome,
            TaskRunOutcome::Completed { checkpoint: None }
        ));
        assert_eq!(
            executor.calls.lock().unwrap().clone(),
            vec![("task-user".to_string(), "wahoo-workout:42".to_string(), 42,)]
        );
    }

    #[test]
    fn parse_errors_are_not_retryable() {
        assert!(!error_is_retryable(&WahooFitEnrichmentError::Parse(
            "bad fit".to_string()
        )));
    }
}
