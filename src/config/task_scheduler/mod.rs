mod maintenance;
mod worker;

use uuid::Uuid;

pub use maintenance::{
    spawn_task_scheduler_maintenance_loop, TaskSchedulerMaintenanceConfig,
    TaskSchedulerWorkerConfig,
};
pub use worker::spawn_task_worker;

pub fn default_task_scheduler_worker_id() -> String {
    resolve_task_scheduler_worker_id(
        std::env::var("TASK_SCHEDULER_WORKER_ID").ok(),
        std::env::var("HOSTNAME").ok(),
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
