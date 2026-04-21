use super::*;

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub fn heartbeat_worker(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            scheduler
                .set_worker_state(worker_id, is_leader, enabled_task_types, active_task_ids)
                .await
        })
    }

    pub fn touch_worker_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            scheduler
                .touch_cached_worker_state(worker_id, is_leader, enabled_task_types)
                .await
        })
    }

    pub fn add_worker_active_task(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        task_id: &str,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let active_task_ids = {
                let mut worker_states = scheduler.worker_states.lock().await;
                let state = worker_states
                    .entry(worker_id.clone())
                    .or_insert_with(|| WorkerState {
                        is_leader,
                        enabled_task_types: enabled_task_types.clone(),
                        active_task_ids: Vec::new(),
                    });
                state.is_leader = is_leader;
                state.enabled_task_types = enabled_task_types.clone();
                if !state
                    .active_task_ids
                    .iter()
                    .any(|active| active == &task_id)
                {
                    state.active_task_ids.push(task_id.clone());
                }
                state.active_task_ids.clone()
            };

            scheduler
                .workers
                .clone()
                .upsert(scheduler.build_task_worker(
                    &worker_id,
                    is_leader,
                    enabled_task_types,
                    active_task_ids,
                ))
                .await
        })
    }

    pub fn remove_worker_active_task(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        task_id: &str,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let active_task_ids = {
                let mut worker_states = scheduler.worker_states.lock().await;
                let state = worker_states
                    .entry(worker_id.clone())
                    .or_insert_with(|| WorkerState {
                        is_leader,
                        enabled_task_types: enabled_task_types.clone(),
                        active_task_ids: Vec::new(),
                    });
                state.is_leader = is_leader;
                state.enabled_task_types = enabled_task_types.clone();
                state.active_task_ids.retain(|active| active != &task_id);
                state.active_task_ids.clone()
            };

            scheduler
                .workers
                .clone()
                .upsert(scheduler.build_task_worker(
                    &worker_id,
                    is_leader,
                    enabled_task_types,
                    active_task_ids,
                ))
                .await
        })
    }

    fn build_task_worker(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> TaskWorker {
        TaskWorker {
            worker_id: worker_id.to_string(),
            is_leader,
            enabled_task_types,
            active_task_ids,
            last_heartbeat_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    async fn set_worker_state(
        &self,
        worker_id: String,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> Result<TaskWorker, TaskSchedulerError> {
        let worker = self.build_task_worker(
            &worker_id,
            is_leader,
            enabled_task_types.clone(),
            active_task_ids.clone(),
        );
        let persisted = self.workers.clone().upsert(worker).await?;
        self.worker_states.lock().await.insert(
            worker_id,
            WorkerState {
                is_leader,
                enabled_task_types,
                active_task_ids,
            },
        );
        Ok(persisted)
    }

    async fn touch_cached_worker_state(
        &self,
        worker_id: String,
        is_leader: bool,
        enabled_task_types: Vec<String>,
    ) -> Result<TaskWorker, TaskSchedulerError> {
        {
            let mut worker_states = self.worker_states.lock().await;
            let state = worker_states
                .entry(worker_id.clone())
                .or_insert_with(|| WorkerState {
                    is_leader,
                    enabled_task_types: enabled_task_types.clone(),
                    active_task_ids: Vec::new(),
                });
            state.is_leader = is_leader;
            state.enabled_task_types = enabled_task_types.clone();
            state.active_task_ids.clear();
        }

        self.workers
            .clone()
            .upsert(self.build_task_worker(&worker_id, is_leader, enabled_task_types, Vec::new()))
            .await
    }
}
