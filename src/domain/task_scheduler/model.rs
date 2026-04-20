use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskSchedulerError {
    Validation(String),
    Conflict(String),
    Repository(String),
}

impl std::fmt::Display for TaskSchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) | Self::Conflict(message) | Self::Repository(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl std::error::Error for TaskSchedulerError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    RetryScheduled,
    Failed,
    Completed,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryStrategy {
    Never,
    Fixed {
        max_attempts: u32,
        delay_seconds: i64,
    },
    Exponential {
        max_attempts: u32,
        initial_delay_seconds: i64,
        max_delay_seconds: i64,
    },
}

impl RetryStrategy {
    pub fn max_attempts(&self) -> u32 {
        match self {
            Self::Never => 1,
            Self::Fixed { max_attempts, .. } | Self::Exponential { max_attempts, .. } => {
                *max_attempts
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledTask {
    pub id: String,
    pub user_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub payload: Value,
    pub checkpoint: Option<Value>,
    pub retry_strategy: RetryStrategy,
    pub dedupe_key: String,
    pub error_message: Option<String>,
    pub attempt_count: u32,
    pub next_attempt_at_epoch_seconds: i64,
    pub claimed_by: Option<String>,
    pub lease_expires_at_epoch_seconds: Option<i64>,
    pub last_heartbeat_at_epoch_seconds: Option<i64>,
    pub execution_timeout_seconds: i64,
    pub timed_out_at_epoch_seconds: Option<i64>,
    pub leader_only: bool,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
    pub started_at_epoch_seconds: Option<i64>,
    pub finished_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewTask {
    pub id: String,
    pub user_id: String,
    pub task_type: String,
    pub payload: Value,
    pub retry_strategy: RetryStrategy,
    pub dedupe_key: String,
    pub execution_timeout_seconds: i64,
    pub leader_only: bool,
}

impl ScheduledTask {
    pub fn new(input: NewTask, now_epoch_seconds: i64) -> Result<Self, TaskSchedulerError> {
        if input.id.trim().is_empty() {
            return Err(TaskSchedulerError::Validation(
                "task id is required".to_string(),
            ));
        }
        if input.user_id.trim().is_empty() {
            return Err(TaskSchedulerError::Validation(
                "task user_id is required".to_string(),
            ));
        }
        if input.task_type.trim().is_empty() {
            return Err(TaskSchedulerError::Validation(
                "task type is required".to_string(),
            ));
        }
        if input.dedupe_key.trim().is_empty() {
            return Err(TaskSchedulerError::Validation(
                "task dedupe_key is required".to_string(),
            ));
        }
        if input.execution_timeout_seconds <= 0 {
            return Err(TaskSchedulerError::Validation(
                "task execution timeout must be positive".to_string(),
            ));
        }

        Ok(Self {
            id: input.id,
            user_id: input.user_id,
            task_type: input.task_type,
            status: TaskStatus::Queued,
            payload: input.payload,
            checkpoint: None,
            retry_strategy: input.retry_strategy,
            dedupe_key: input.dedupe_key,
            error_message: None,
            attempt_count: 0,
            next_attempt_at_epoch_seconds: now_epoch_seconds,
            claimed_by: None,
            lease_expires_at_epoch_seconds: None,
            last_heartbeat_at_epoch_seconds: None,
            execution_timeout_seconds: input.execution_timeout_seconds,
            timed_out_at_epoch_seconds: None,
            leader_only: input.leader_only,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            started_at_epoch_seconds: None,
            finished_at_epoch_seconds: None,
        })
    }

    pub fn is_timeout_candidate(&self, now_epoch_seconds: i64) -> bool {
        if self.status != TaskStatus::Running {
            return false;
        }

        if self
            .lease_expires_at_epoch_seconds
            .is_some_and(|lease| lease <= now_epoch_seconds)
        {
            return true;
        }

        self.started_at_epoch_seconds
            .is_some_and(|started| started + self.execution_timeout_seconds <= now_epoch_seconds)
    }

    pub fn can_retry_manually(&self) -> bool {
        matches!(self.status, TaskStatus::Failed | TaskStatus::TimedOut)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskWorker {
    pub worker_id: String,
    pub is_leader: bool,
    pub enabled_task_types: Vec<String>,
    pub active_task_ids: Vec<String>,
    pub last_heartbeat_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskEnqueueResult {
    pub task: ScheduledTask,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskClaimRequest {
    pub worker_id: String,
    pub enabled_task_types: Vec<String>,
    pub is_leader: bool,
    pub now_epoch_seconds: i64,
    pub lease_expires_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskHeartbeatRequest {
    pub task_id: String,
    pub worker_id: String,
    pub last_heartbeat_at_epoch_seconds: i64,
    pub lease_expires_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskMarkTimedOutRequest {
    pub task_id: String,
    pub expected_claimed_by: Option<String>,
    pub expected_updated_at_epoch_seconds: i64,
    pub timed_out_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRecoverRequest {
    pub task_id: String,
    pub expected_claimed_by: Option<String>,
    pub expected_updated_at_epoch_seconds: i64,
    pub recovered_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRetryRequest {
    pub task_id: String,
    pub retried_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskListFilter {
    pub task_types: Vec<String>,
    pub statuses: Vec<TaskStatus>,
    pub user_id: Option<String>,
}
