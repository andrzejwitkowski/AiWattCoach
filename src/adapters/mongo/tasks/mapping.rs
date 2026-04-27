use mongodb::bson::DateTime;

use crate::domain::task_scheduler::{RetryStrategy, ScheduledTask, TaskSchedulerError, TaskStatus};

use super::document::{RetryStrategyDocument, TaskDocument};

// 60 days
const TERMINAL_TASK_TTL_SECONDS: i64 = 60 * 24 * 60 * 60;

pub(super) fn storage_error(error: mongodb::error::Error) -> TaskSchedulerError {
    TaskSchedulerError::Repository(error.to_string())
}

pub(super) fn status_as_str(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::RetryScheduled => "retry_scheduled",
        TaskStatus::Failed => "failed",
        TaskStatus::Completed => "completed",
        TaskStatus::TimedOut => "timed_out",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn map_status(value: &str) -> Result<TaskStatus, TaskSchedulerError> {
    match value {
        "queued" => Ok(TaskStatus::Queued),
        "running" => Ok(TaskStatus::Running),
        "retry_scheduled" => Ok(TaskStatus::RetryScheduled),
        "failed" => Ok(TaskStatus::Failed),
        "completed" => Ok(TaskStatus::Completed),
        "timed_out" => Ok(TaskStatus::TimedOut),
        "cancelled" => Ok(TaskStatus::Cancelled),
        other => Err(TaskSchedulerError::Repository(format!(
            "unknown task status: {other}",
        ))),
    }
}

fn map_retry_strategy(strategy: &RetryStrategy) -> RetryStrategyDocument {
    match strategy {
        RetryStrategy::Never => RetryStrategyDocument {
            kind: "never".to_string(),
            max_attempts: Some(1),
            delay_seconds: None,
            initial_delay_seconds: None,
            max_delay_seconds: None,
        },
        RetryStrategy::Fixed {
            max_attempts,
            delay_seconds,
        } => RetryStrategyDocument {
            kind: "fixed".to_string(),
            max_attempts: Some(i64::from(*max_attempts)),
            delay_seconds: Some(*delay_seconds),
            initial_delay_seconds: None,
            max_delay_seconds: None,
        },
        RetryStrategy::Exponential {
            max_attempts,
            initial_delay_seconds,
            max_delay_seconds,
        } => RetryStrategyDocument {
            kind: "exponential".to_string(),
            max_attempts: Some(i64::from(*max_attempts)),
            delay_seconds: None,
            initial_delay_seconds: Some(*initial_delay_seconds),
            max_delay_seconds: Some(*max_delay_seconds),
        },
    }
}

fn map_retry_strategy_document(
    document: RetryStrategyDocument,
) -> Result<RetryStrategy, TaskSchedulerError> {
    let strategy = match document.kind.as_str() {
        "never" => Ok(RetryStrategy::Never),
        "fixed" => Ok(RetryStrategy::Fixed {
            max_attempts: parse_u32_field(document.max_attempts, "fixed retry max_attempts")?,
            delay_seconds: document.delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "fixed retry strategy missing delay_seconds".to_string(),
                )
            })?,
        }),
        "exponential" => Ok(RetryStrategy::Exponential {
            max_attempts: parse_u32_field(document.max_attempts, "exponential retry max_attempts")?,
            initial_delay_seconds: document.initial_delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "exponential retry strategy missing initial_delay_seconds".to_string(),
                )
            })?,
            max_delay_seconds: document.max_delay_seconds.ok_or_else(|| {
                TaskSchedulerError::Repository(
                    "exponential retry strategy missing max_delay_seconds".to_string(),
                )
            })?,
        }),
        other => Err(TaskSchedulerError::Repository(format!(
            "unknown retry strategy kind: {other}",
        ))),
    }?;

    validate_retry_strategy(&strategy)?;
    Ok(strategy)
}

fn validate_retry_strategy(strategy: &RetryStrategy) -> Result<(), TaskSchedulerError> {
    match strategy {
        RetryStrategy::Never => Ok(()),
        RetryStrategy::Fixed {
            max_attempts,
            delay_seconds,
        } => {
            if *max_attempts == 0 {
                return Err(TaskSchedulerError::Repository(
                    "fixed retry strategy max_attempts must be positive".to_string(),
                ));
            }
            if *delay_seconds <= 0 {
                return Err(TaskSchedulerError::Repository(
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
                return Err(TaskSchedulerError::Repository(
                    "exponential retry strategy max_attempts must be positive".to_string(),
                ));
            }
            if *initial_delay_seconds <= 0 {
                return Err(TaskSchedulerError::Repository(
                    "exponential retry strategy initial_delay_seconds must be positive".to_string(),
                ));
            }
            if *max_delay_seconds <= 0 || *max_delay_seconds < *initial_delay_seconds {
                return Err(TaskSchedulerError::Repository(
                    "exponential retry strategy max_delay_seconds must be positive and >= initial_delay_seconds".to_string(),
                ));
            }
            Ok(())
        }
    }
}

fn parse_u32_field(value: Option<i64>, field_name: &str) -> Result<u32, TaskSchedulerError> {
    let value = value.ok_or_else(|| {
        TaskSchedulerError::Repository(format!("missing {field_name} in task retry strategy"))
    })?;
    u32::try_from(value)
        .map_err(|_| TaskSchedulerError::Repository(format!("invalid {field_name}: {value}")))
}

pub(super) fn map_task_to_document(
    task: &ScheduledTask,
) -> Result<TaskDocument, TaskSchedulerError> {
    validate_retry_strategy(&task.retry_strategy)?;

    Ok(TaskDocument {
        id: task.id.clone(),
        user_id: task.user_id.clone(),
        task_type: task.task_type.clone(),
        status: status_as_str(&task.status).to_string(),
        payload: task.payload.clone(),
        checkpoint: task.checkpoint.clone(),
        retry_strategy: map_retry_strategy(&task.retry_strategy),
        dedupe_key: task.dedupe_key.clone(),
        error_message: task.error_message.clone(),
        attempt_count: i64::from(task.attempt_count),
        next_attempt_at_epoch_seconds: task.next_attempt_at_epoch_seconds,
        claimed_by: task.claimed_by.clone(),
        lease_expires_at_epoch_seconds: task.lease_expires_at_epoch_seconds,
        last_heartbeat_at_epoch_seconds: task.last_heartbeat_at_epoch_seconds,
        execution_timeout_seconds: task.execution_timeout_seconds,
        timed_out_at_epoch_seconds: task.timed_out_at_epoch_seconds,
        leader_only: task.leader_only,
        created_at_epoch_seconds: task.created_at_epoch_seconds,
        updated_at_epoch_seconds: task.updated_at_epoch_seconds,
        started_at_epoch_seconds: task.started_at_epoch_seconds,
        finished_at_epoch_seconds: task.finished_at_epoch_seconds,
        cleanup_after: terminal_task_cleanup_after(&task.status, task.finished_at_epoch_seconds)?,
    })
}

pub(super) fn map_document_to_task(
    document: TaskDocument,
) -> Result<ScheduledTask, TaskSchedulerError> {
    if document.execution_timeout_seconds <= 0 {
        return Err(TaskSchedulerError::Repository(
            "task execution timeout must be positive".to_string(),
        ));
    }

    Ok(ScheduledTask {
        id: document.id,
        user_id: document.user_id,
        task_type: document.task_type,
        status: map_status(&document.status)?,
        payload: document.payload,
        checkpoint: document.checkpoint,
        retry_strategy: map_retry_strategy_document(document.retry_strategy)?,
        dedupe_key: document.dedupe_key,
        error_message: document.error_message,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            TaskSchedulerError::Repository("invalid task attempt_count".to_string())
        })?,
        next_attempt_at_epoch_seconds: document.next_attempt_at_epoch_seconds,
        claimed_by: document.claimed_by,
        lease_expires_at_epoch_seconds: document.lease_expires_at_epoch_seconds,
        last_heartbeat_at_epoch_seconds: document.last_heartbeat_at_epoch_seconds,
        execution_timeout_seconds: document.execution_timeout_seconds,
        timed_out_at_epoch_seconds: document.timed_out_at_epoch_seconds,
        leader_only: document.leader_only,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        started_at_epoch_seconds: document.started_at_epoch_seconds,
        finished_at_epoch_seconds: document.finished_at_epoch_seconds,
    })
}

pub(super) fn terminal_task_cleanup_bson(
    status: &TaskStatus,
    finished_at_epoch_seconds: i64,
) -> Result<mongodb::bson::Bson, TaskSchedulerError> {
    Ok(mongodb::bson::Bson::DateTime(
        terminal_task_cleanup_after(status, Some(finished_at_epoch_seconds))?.ok_or_else(|| {
            TaskSchedulerError::Repository(
                "terminal task cleanup date requires a finished_at timestamp".to_string(),
            )
        })?,
    ))
}

fn terminal_task_cleanup_after(
    status: &TaskStatus,
    finished_at_epoch_seconds: Option<i64>,
) -> Result<Option<DateTime>, TaskSchedulerError> {
    let is_terminal = matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::TimedOut
    );
    if !is_terminal {
        return Ok(None);
    }

    let Some(finished_at_epoch_seconds) = finished_at_epoch_seconds else {
        return Err(TaskSchedulerError::Repository(
            "terminal task cleanup date requires a finished_at timestamp".to_string(),
        ));
    };
    let cleanup_at_epoch_seconds = finished_at_epoch_seconds
        .checked_add(TERMINAL_TASK_TTL_SECONDS)
        .ok_or_else(|| {
            TaskSchedulerError::Repository(
                "task cleanup timestamp exceeds BSON DateTime range".to_string(),
            )
        })?;
    let cleanup_at_epoch_millis = cleanup_at_epoch_seconds.checked_mul(1000).ok_or_else(|| {
        TaskSchedulerError::Repository(
            "task cleanup timestamp exceeds BSON DateTime range".to_string(),
        )
    })?;

    Ok(Some(DateTime::from_millis(cleanup_at_epoch_millis)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn map_task_to_document_rejects_terminal_task_without_finished_at() {
        let error = map_task_to_document(&sample_task(TaskStatus::Completed, None))
            .expect_err("terminal task without finished_at should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Repository(
                "terminal task cleanup date requires a finished_at timestamp".to_string(),
            )
        );
    }

    #[test]
    fn map_task_to_document_skips_cleanup_after_for_non_terminal_task_without_finished_at() {
        let document = map_task_to_document(&sample_task(TaskStatus::Queued, None))
            .expect("non-terminal task should map successfully");

        assert_eq!(document.cleanup_after, None);
    }

    #[test]
    fn map_task_to_document_sets_cleanup_after_for_terminal_task() {
        let finished_at_epoch_seconds = 100;
        let document = map_task_to_document(&sample_task(
            TaskStatus::Completed,
            Some(finished_at_epoch_seconds),
        ))
        .expect("terminal task should map successfully");

        assert_eq!(
            document.cleanup_after,
            Some(DateTime::from_millis(
                (finished_at_epoch_seconds + TERMINAL_TASK_TTL_SECONDS) * 1000,
            ))
        );
    }

    #[test]
    fn map_document_to_task_rejects_non_positive_execution_timeout() {
        let mut document = sample_task_document();
        document.execution_timeout_seconds = 0;

        let error = map_document_to_task(document)
            .expect_err("non-positive execution timeout should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Repository("task execution timeout must be positive".to_string())
        );
    }

    #[test]
    fn map_document_to_task_rejects_invalid_fixed_retry_strategy() {
        let mut document = sample_task_document();
        document.retry_strategy = RetryStrategyDocument {
            kind: "fixed".to_string(),
            max_attempts: Some(0),
            delay_seconds: Some(30),
            initial_delay_seconds: None,
            max_delay_seconds: None,
        };

        let error = map_document_to_task(document)
            .expect_err("invalid fixed retry strategy should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Repository(
                "fixed retry strategy max_attempts must be positive".to_string()
            )
        );
    }

    #[test]
    fn map_task_to_document_rejects_invalid_fixed_retry_strategy() {
        let error = map_task_to_document(&ScheduledTask {
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 0,
                delay_seconds: 30,
            },
            ..sample_task(TaskStatus::Queued, None)
        })
        .expect_err("invalid fixed retry strategy should be rejected before persistence");

        assert_eq!(
            error,
            TaskSchedulerError::Repository(
                "fixed retry strategy max_attempts must be positive".to_string()
            )
        );
    }

    #[test]
    fn map_document_to_task_rejects_invalid_exponential_retry_strategy() {
        let mut document = sample_task_document();
        document.retry_strategy = RetryStrategyDocument {
            kind: "exponential".to_string(),
            max_attempts: Some(2),
            delay_seconds: None,
            initial_delay_seconds: Some(60),
            max_delay_seconds: Some(30),
        };

        let error = map_document_to_task(document)
            .expect_err("invalid exponential retry strategy should be rejected");

        assert_eq!(
            error,
            TaskSchedulerError::Repository(
                "exponential retry strategy max_delay_seconds must be positive and >= initial_delay_seconds".to_string()
            )
        );
    }

    fn sample_task_document() -> TaskDocument {
        TaskDocument {
            id: "task-1".to_string(),
            user_id: "user-1".to_string(),
            task_type: "summary".to_string(),
            status: "queued".to_string(),
            payload: json!({"task": "task-1"}),
            checkpoint: None,
            retry_strategy: RetryStrategyDocument {
                kind: "never".to_string(),
                max_attempts: Some(1),
                delay_seconds: None,
                initial_delay_seconds: None,
                max_delay_seconds: None,
            },
            dedupe_key: "dedupe-1".to_string(),
            error_message: None,
            attempt_count: 0,
            next_attempt_at_epoch_seconds: 100,
            claimed_by: None,
            lease_expires_at_epoch_seconds: None,
            last_heartbeat_at_epoch_seconds: None,
            execution_timeout_seconds: 30,
            timed_out_at_epoch_seconds: None,
            leader_only: false,
            created_at_epoch_seconds: 100,
            updated_at_epoch_seconds: 100,
            started_at_epoch_seconds: None,
            finished_at_epoch_seconds: None,
            cleanup_after: None,
        }
    }

    fn sample_task(status: TaskStatus, finished_at_epoch_seconds: Option<i64>) -> ScheduledTask {
        ScheduledTask {
            id: "task-1".to_string(),
            user_id: "user-1".to_string(),
            task_type: "summary".to_string(),
            status,
            payload: json!({"task": "task-1"}),
            checkpoint: None,
            retry_strategy: RetryStrategy::Never,
            dedupe_key: "dedupe-1".to_string(),
            error_message: None,
            attempt_count: 0,
            next_attempt_at_epoch_seconds: 100,
            claimed_by: None,
            lease_expires_at_epoch_seconds: None,
            last_heartbeat_at_epoch_seconds: None,
            execution_timeout_seconds: 30,
            timed_out_at_epoch_seconds: None,
            leader_only: false,
            created_at_epoch_seconds: 100,
            updated_at_epoch_seconds: 100,
            started_at_epoch_seconds: None,
            finished_at_epoch_seconds,
        }
    }
}
