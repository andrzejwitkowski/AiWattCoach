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
            let mut worker_states = scheduler.worker_states.clone().lock_owned().await;
            let (worker, previous_state) = {
                let previous_state = worker_states.get(&worker_id).cloned();
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
                (
                    scheduler.build_task_worker(
                        &worker_id,
                        is_leader,
                        enabled_task_types,
                        state.active_task_ids.clone(),
                    ),
                    previous_state,
                )
            };

            match scheduler.workers.clone().upsert(worker).await {
                Ok(persisted) => Ok(persisted),
                Err(error) => {
                    restore_worker_state(&mut worker_states, worker_id, previous_state);
                    Err(error)
                }
            }
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
            let mut worker_states = scheduler.worker_states.clone().lock_owned().await;
            let (worker, previous_state) = {
                let previous_state = worker_states.get(&worker_id).cloned();
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
                (
                    scheduler.build_task_worker(
                        &worker_id,
                        is_leader,
                        enabled_task_types,
                        state.active_task_ids.clone(),
                    ),
                    previous_state,
                )
            };

            match scheduler.workers.clone().upsert(worker).await {
                Ok(persisted) => Ok(persisted),
                Err(error) => {
                    restore_worker_state(&mut worker_states, worker_id, previous_state);
                    Err(error)
                }
            }
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
        let mut worker_states = self.worker_states.clone().lock_owned().await;
        let previous_state = {
            let previous_state = worker_states.get(&worker_id).cloned();
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
            previous_state
        };

        match self
            .workers
            .clone()
            .upsert(self.build_task_worker(&worker_id, is_leader, enabled_task_types, Vec::new()))
            .await
        {
            Ok(persisted) => Ok(persisted),
            Err(error) => {
                restore_worker_state(&mut worker_states, worker_id, previous_state);
                Err(error)
            }
        }
    }
}

fn restore_worker_state(
    worker_states: &mut tokio::sync::OwnedMutexGuard<HashMap<String, WorkerState>>,
    worker_id: String,
    previous_state: Option<WorkerState>,
) {
    if let Some(previous_state) = previous_state {
        worker_states.insert(worker_id, previous_state);
    } else {
        worker_states.remove(&worker_id);
    }
}
