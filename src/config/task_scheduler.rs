use std::{env, time::Duration};

use tokio::time::MissedTickBehavior;
use tracing::warn;
use uuid::Uuid;

use crate::domain::{
    identity::Clock,
    task_scheduler::{TaskRepository, TaskSchedulerService, TaskWorkerRepository},
};

const DEFAULT_WORKER_HEARTBEAT_INTERVAL_SECONDS: u64 = 15;
const DEFAULT_TIMEOUT_SWEEP_INTERVAL_SECONDS: u64 = 30;
const DEFAULT_WORKER_STALE_AFTER_SECONDS: i64 = 30 * 60;
const DEFAULT_TIMEOUT_SWEEP_LIMIT: usize = 100;

pub fn default_task_scheduler_worker_id() -> String {
    resolve_task_scheduler_worker_id(
        env::var("TASK_SCHEDULER_WORKER_ID").ok(),
        env::var("HOSTNAME").ok(),
    )
}

fn resolve_task_scheduler_worker_id(
    explicit_worker_id: Option<String>,
    hostname: Option<String>,
) -> String {
    if let Some(worker_id) = explicit_worker_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return worker_id;
    }

    if let Some(hostname) = hostname
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return format!("task-scheduler-{hostname}");
    }

    format!("task-scheduler-{}", Uuid::new_v4())
}

#[derive(Clone, Debug)]
pub struct TaskSchedulerWorkerConfig {
    pub worker_id: String,
    pub is_leader: bool,
    pub enabled_task_types: Vec<String>,
}

impl TaskSchedulerWorkerConfig {
    pub fn new(worker_id: String, is_leader: bool, enabled_task_types: Vec<String>) -> Self {
        Self {
            worker_id,
            is_leader,
            enabled_task_types,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TaskSchedulerMaintenanceConfig {
    pub worker_heartbeat_interval_seconds: u64,
    pub timeout_sweep_interval_seconds: u64,
    pub worker_stale_after_seconds: i64,
    pub timeout_sweep_limit: usize,
}

impl Default for TaskSchedulerMaintenanceConfig {
    fn default() -> Self {
        Self {
            worker_heartbeat_interval_seconds: DEFAULT_WORKER_HEARTBEAT_INTERVAL_SECONDS,
            timeout_sweep_interval_seconds: DEFAULT_TIMEOUT_SWEEP_INTERVAL_SECONDS,
            worker_stale_after_seconds: DEFAULT_WORKER_STALE_AFTER_SECONDS,
            timeout_sweep_limit: DEFAULT_TIMEOUT_SWEEP_LIMIT,
        }
    }
}

pub fn spawn_task_scheduler_maintenance_loop<Tasks, Workers, Time>(
    service: TaskSchedulerService<Tasks, Workers, Time>,
    worker: TaskSchedulerWorkerConfig,
    config: TaskSchedulerMaintenanceConfig,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    tokio::spawn(async move {
        let mut worker_ticker = tokio::time::interval(Duration::from_secs(
            config.worker_heartbeat_interval_seconds,
        ));
        worker_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut timeout_ticker =
            tokio::time::interval(Duration::from_secs(config.timeout_sweep_interval_seconds));
        timeout_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = worker_ticker.tick() => {
                    if let Err(error) = service
                        .touch_worker_heartbeat(
                            &worker.worker_id,
                            worker.is_leader,
                            worker.enabled_task_types.clone(),
                        )
                        .await
                    {
                        warn!(worker_id = %worker.worker_id, %error, "task scheduler failed to persist worker heartbeat");
                    }
                }
                _ = timeout_ticker.tick() => {
                    if let Err(error) = service
                        .sweep_timed_out_tasks(
                            config.worker_stale_after_seconds,
                            config.timeout_sweep_limit,
                        )
                        .await
                    {
                        warn!(worker_id = %worker.worker_id, %error, "task scheduler timeout sweep failed");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::resolve_task_scheduler_worker_id;

    #[test]
    fn default_worker_id_prefers_explicit_env() {
        let worker_id = resolve_task_scheduler_worker_id(
            Some("scheduler-a".to_string()),
            Some("container-1".to_string()),
        );

        assert_eq!(worker_id, "scheduler-a");
    }

    #[test]
    fn default_worker_id_falls_back_to_hostname() {
        let worker_id = resolve_task_scheduler_worker_id(None, Some("container-2".to_string()));

        assert_eq!(worker_id, "task-scheduler-container-2");
    }

    #[test]
    fn default_worker_id_falls_back_to_uuid_when_env_missing() {
        let worker_id = resolve_task_scheduler_worker_id(None, None);

        assert!(worker_id.starts_with("task-scheduler-"));
    }
}
