use std::sync::Arc;

use tokio::sync::Notify;

use crate::domain::task_scheduler::{BoxFuture, ScheduledTask, TaskHandler, TaskRunOutcome};

pub struct StaticTaskHandler {
    task_type: &'static str,
}

impl StaticTaskHandler {
    pub fn new(task_type: &'static str) -> Arc<Self> {
        Arc::new(Self { task_type })
    }
}

impl TaskHandler for StaticTaskHandler {
    fn task_type(&self) -> &'static str {
        self.task_type
    }

    fn run(&self, _task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        Box::pin(async { TaskRunOutcome::Completed { checkpoint: None } })
    }
}

pub struct PanicTaskHandler {
    pub started: Arc<Notify>,
    pub release: Arc<Notify>,
}

impl PanicTaskHandler {
    pub const TASK_TYPE: &'static str = "panic.task";

    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        })
    }
}

impl TaskHandler for PanicTaskHandler {
    fn task_type(&self) -> &'static str {
        Self::TASK_TYPE
    }

    fn run(&self, _task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let started = self.started.clone();
        let release = self.release.clone();
        Box::pin(async move {
            started.notify_one();
            release.notified().await;
            panic!("panic task handler boom");
        })
    }
}
