use std::{sync::Arc, time::Duration};

use super::{BoxFuture, ScheduledTask};

#[derive(Clone, Debug)]
pub struct TaskWorkerConfig {
    pub is_leader: bool,
    pub lease_duration_seconds: i64,
    pub heartbeat_interval: Duration,
    pub idle_poll_interval: Duration,
    pub max_concurrency: usize,
}

#[derive(Debug)]
pub enum TaskRunOutcome {
    Completed {
        checkpoint: Option<serde_json::Value>,
    },
    Failed {
        checkpoint: Option<serde_json::Value>,
        error_message: String,
        retryable: bool,
    },
}

pub trait TaskHandler: Send + Sync + 'static {
    fn task_type(&self) -> &'static str;

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome>;
}

pub type SharedTaskHandler = Arc<dyn TaskHandler>;
