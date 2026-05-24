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

    pub fn next_retry_at(&self, attempt_count: u32, now_epoch_seconds: i64) -> Option<i64> {
        if attempt_count >= self.max_attempts() {
            return None;
        }

        let delay_seconds = match self {
            Self::Never => return None,
            Self::Fixed { delay_seconds, .. } => *delay_seconds,
            Self::Exponential {
                initial_delay_seconds,
                max_delay_seconds,
                ..
            } => {
                let exponent = attempt_count.saturating_sub(1).min(30);
                let multiplier = 1_i64.checked_shl(exponent).unwrap_or(i64::MAX);
                initial_delay_seconds
                    .saturating_mul(multiplier)
                    .min(*max_delay_seconds)
            }
        };

        Some(now_epoch_seconds.saturating_add(delay_seconds))
    }
}

fn validate_retry_strategy(strategy: &RetryStrategy) -> Result<(), TaskSchedulerError> {
    match strategy {
        RetryStrategy::Never => Ok(()),
        RetryStrategy::Fixed {
            max_attempts,
            delay_seconds,
        } => {
            if *max_attempts == 0 {
                return Err(TaskSchedulerError::Validation(
                    "fixed retry strategy max_attempts must be positive".to_string(),
                ));
            }
            if *delay_seconds <= 0 {
                return Err(TaskSchedulerError::Validation(
                    "fixed retry strategy delay_seconds must be positive".to_string(),
                ));
            }
            Ok(())
        }
        RetryStrategy::Exponential {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } => {
            if *max_attempts == 0 {
                return Err(TaskSchedulerError::Validation(
                    "exponential retry strategy max_attempts must be positive".to_string(),
                ));
            }
            if *initial_delay_seconds <= 0 {
                return Err(TaskSchedulerError::Validation(
                    "exponential retry strategy initial_delay_seconds must be positive".to_string(),
                ));
            }
            if *max_delay_seconds <= 0 {
                return Err(TaskSchedulerError::Validation(
                    "exponential retry strategy max_delay_seconds must be positive".to_string(),
                ));
            }
            if *max_delay_seconds < *initial_delay_seconds {
                return Err(TaskSchedulerError::Validation(
                    "exponential retry strategy max_delay_seconds must be greater than or equal to initial_delay_seconds".to_string(),
                ));
            }
            Ok(())
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
        validate_retry_strategy(&input.retry_strategy)?;

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

#[derive(Clone, Debug, PartialEq)]
pub struct TaskCheckpointRequest {
    pub task_id: String,
    pub worker_id: String,
    pub checkpoint: Value,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskCompleteRequest {
    pub task_id: String,
    pub worker_id: String,
    pub checkpoint: Option<Value>,
    pub completed_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskFailRequest {
    pub task_id: String,
    pub worker_id: String,
    pub checkpoint: Option<Value>,
    pub error_message: String,
    pub failed_at_epoch_seconds: i64,
    pub retry_at_epoch_seconds: Option<i64>,
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

pub const DEFAULT_TASK_LIST_LIMIT: usize = 20;
pub const MAX_TASK_LIST_LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSortField {
    Id,
    UserId,
    TaskType,
    Status,
    DedupeKey,
    ErrorMessage,
    AttemptCount,
    NextAttemptAt,
    ClaimedBy,
    LeaseExpiresAt,
    LastHeartbeatAt,
    ExecutionTimeout,
    TimedOutAt,
    LeaderOnly,
    #[default]
    CreatedAt,
    UpdatedAt,
    StartedAt,
    FinishedAt,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskListFilter {
    pub task_types: Vec<String>,
    pub statuses: Vec<TaskStatus>,
    pub user_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub sort_field: TaskSortField,
    pub sort_direction: TaskSortDirection,
}

impl TaskListFilter {
    pub fn clamped_limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_TASK_LIST_LIMIT)
            .clamp(1, MAX_TASK_LIST_LIMIT)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskListPage {
    pub tasks: Vec<ScheduledTask>,
    pub has_next_page: bool,
}
