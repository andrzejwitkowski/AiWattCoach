use std::time::Duration;

use tokio::time::MissedTickBehavior;
use tracing::warn;

use crate::{
    domain::{
        identity::Clock,
        task_scheduler::{TaskRepository, TaskSchedulerService, TaskWorkerRepository},
    },
    BackgroundTaskHandle,
};

const DEFAULT_WORKER_HEARTBEAT_INTERVAL_SECONDS: u64 = 15;
const DEFAULT_TIMEOUT_SWEEP_INTERVAL_SECONDS: u64 = 30;
const DEFAULT_WORKER_STALE_AFTER_SECONDS: i64 = 5 * 60;
const DEFAULT_TIMEOUT_SWEEP_LIMIT: usize = 100;

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
) -> Result<BackgroundTaskHandle, crate::domain::task_scheduler::TaskSchedulerError>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    validate_task_scheduler_maintenance_config(&config)?;
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let join_handle = tokio::spawn(async move {
        let mut worker_ticker = tokio::time::interval(Duration::from_secs(
            config.worker_heartbeat_interval_seconds,
        ));
        worker_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let mut timeout_ticker =
            tokio::time::interval(Duration::from_secs(config.timeout_sweep_interval_seconds));
        timeout_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    break;
                }
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

    Ok(BackgroundTaskHandle::new(
        "task-scheduler-maintenance",
        shutdown_tx,
        join_handle,
    ))
}

fn validate_task_scheduler_maintenance_config(
    config: &TaskSchedulerMaintenanceConfig,
) -> Result<(), crate::domain::task_scheduler::TaskSchedulerError> {
    if config.worker_heartbeat_interval_seconds == 0 {
        return Err(
            crate::domain::task_scheduler::TaskSchedulerError::Validation(
                "task scheduler worker_heartbeat_interval_seconds must be positive".to_string(),
            ),
        );
    }

    if config.timeout_sweep_interval_seconds == 0 {
        return Err(
            crate::domain::task_scheduler::TaskSchedulerError::Validation(
                "task scheduler timeout_sweep_interval_seconds must be positive".to_string(),
            ),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task_scheduler::TaskSchedulerError;

    #[test]
    fn maintenance_config_accepts_defaults() {
        assert!(validate_task_scheduler_maintenance_config(
            &TaskSchedulerMaintenanceConfig::default()
        )
        .is_ok());
    }

    #[test]
    fn maintenance_config_rejects_zero_worker_heartbeat_interval() {
        let error = validate_task_scheduler_maintenance_config(&TaskSchedulerMaintenanceConfig {
            worker_heartbeat_interval_seconds: 0,
            ..TaskSchedulerMaintenanceConfig::default()
        })
        .expect_err("zero worker heartbeat interval should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Validation(
                "task scheduler worker_heartbeat_interval_seconds must be positive".to_string(),
            )
        );
    }

    #[test]
    fn maintenance_config_rejects_zero_timeout_sweep_interval() {
        let error = validate_task_scheduler_maintenance_config(&TaskSchedulerMaintenanceConfig {
            timeout_sweep_interval_seconds: 0,
            ..TaskSchedulerMaintenanceConfig::default()
        })
        .expect_err("zero timeout sweep interval should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Validation(
                "task scheduler timeout_sweep_interval_seconds must be positive".to_string(),
            )
        );
    }
}
