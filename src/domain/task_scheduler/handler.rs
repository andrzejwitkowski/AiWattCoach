use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use super::{BoxFuture, NewTask, RetryStrategy, ScheduledTask, TaskHandler, TaskRunOutcome};

pub(crate) struct NewScheduledTaskInput<Payload> {
    pub id: String,
    pub user_id: String,
    pub task_type: &'static str,
    pub payload: Payload,
    pub retry_strategy: RetryStrategy,
    pub dedupe_key: String,
    pub execution_timeout_seconds: i64,
    pub leader_only: bool,
    pub now_epoch_seconds: i64,
}

pub(crate) trait ScheduledTaskExecutor: Send + Sync + 'static {
    type Payload: DeserializeOwned + Send + 'static;
    type Output: Send + 'static;
    type Error: std::fmt::Display + Send + 'static;

    fn task_type(&self) -> &'static str;

    fn parse_error(&self, error: serde_json::Error) -> Self::Error;

    fn on_parse_error(&self, _task: &ScheduledTask, _error: &Self::Error) {}

    fn run(
        &self,
        task: ScheduledTask,
        payload: Self::Payload,
    ) -> BoxFuture<Result<Self::Output, Self::Error>>;

    fn completed_checkpoint(
        &self,
        task_id: &str,
        output: &Self::Output,
    ) -> Result<Option<serde_json::Value>, Self::Error>;

    fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome;
}

pub(crate) struct ScheduledTaskHandler<Executor> {
    executor: Arc<Executor>,
}

pub(crate) fn scheduled_task_handler<Executor>(executor: Executor) -> ScheduledTaskHandler<Executor>
where
    Executor: ScheduledTaskExecutor,
{
    ScheduledTaskHandler {
        executor: Arc::new(executor),
    }
}

pub(crate) fn build_scheduled_task<Payload>(
    input: NewScheduledTaskInput<Payload>,
) -> Result<ScheduledTask, BuildScheduledTaskError>
where
    Payload: Serialize,
{
    let payload =
        serde_json::to_value(input.payload).map_err(BuildScheduledTaskError::SerializePayload)?;

    ScheduledTask::new(
        NewTask {
            id: input.id,
            user_id: input.user_id,
            task_type: input.task_type.to_string(),
            payload,
            retry_strategy: input.retry_strategy,
            dedupe_key: input.dedupe_key,
            execution_timeout_seconds: input.execution_timeout_seconds,
            leader_only: input.leader_only,
        },
        input.now_epoch_seconds,
    )
    .map_err(BuildScheduledTaskError::Scheduler)
}

pub(crate) fn parse_optional_json_value<T, E>(
    value: Option<Value>,
    invalid_message: &str,
    map_error: fn(String) -> E,
) -> Result<Option<T>, E>
where
    T: DeserializeOwned,
{
    value
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|error| map_error(format!("{invalid_message}: {error}")))
        })
        .transpose()
}

pub(crate) fn parse_required_json_value<T, E>(
    value: Option<Value>,
    missing_message: &str,
    invalid_message: &str,
    map_error: fn(String) -> E,
) -> Result<T, E>
where
    T: DeserializeOwned,
{
    parse_optional_json_value(value, invalid_message, map_error)?
        .ok_or_else(|| map_error(missing_message.to_string()))
}

pub(crate) fn parse_failed_or_error_message<E>(
    parsed_error: Option<E>,
    error_message: Option<String>,
    missing_error_message: &str,
    map_error: fn(String) -> E,
) -> E {
    parsed_error
        .or_else(|| error_message.map(map_error))
        .unwrap_or_else(|| map_error(missing_error_message.to_string()))
}

pub(crate) fn serialize_json_value<T, E>(
    value: &T,
    serialize_error_message: &str,
    map_error: fn(String) -> E,
) -> Result<Value, E>
where
    T: Serialize,
{
    serde_json::to_value(value)
        .map_err(|error| map_error(format!("{serialize_error_message}: {error}")))
}

#[derive(Debug)]
pub(crate) enum BuildScheduledTaskError {
    SerializePayload(serde_json::Error),
    Scheduler(super::TaskSchedulerError),
}

impl std::fmt::Display for BuildScheduledTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializePayload(error) => write!(f, "{error}"),
            Self::Scheduler(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BuildScheduledTaskError {}

impl<Executor> TaskHandler for ScheduledTaskHandler<Executor>
where
    Executor: ScheduledTaskExecutor,
{
    fn task_type(&self) -> &'static str {
        self.executor.task_type()
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let parse_result = serde_json::from_value::<Executor::Payload>(task.payload.clone())
            .map_err(|error| self.executor.parse_error(error));

        match parse_result {
            Ok(payload) => {
                let executor = Arc::clone(&self.executor);
                let task_id = task.id.clone();
                Box::pin(async move {
                    match executor.run(task, payload).await {
                        Ok(output) => match executor.completed_checkpoint(&task_id, &output) {
                            Ok(checkpoint) => TaskRunOutcome::Completed { checkpoint },
                            Err(error) => executor.failed_outcome(error),
                        },
                        Err(error) => executor.failed_outcome(error),
                    }
                })
            }
            Err(error) => {
                self.executor.on_parse_error(&task, &error);
                let outcome = TaskRunOutcome::Failed {
                    checkpoint: None,
                    error_message: error.to_string(),
                    retryable: false,
                    retry_delay_seconds: None,
                };
                Box::pin(async move { outcome })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::*;
    use crate::domain::task_scheduler::{RetryStrategy, TaskSchedulerError};

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct StubError {
        message: String,
        retryable: bool,
        retry_delay_seconds: Option<i64>,
    }

    impl std::fmt::Display for StubError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    #[derive(Clone, Deserialize, Serialize)]
    struct StubPayload {
        value: String,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    struct StubCheckpoint {
        value: String,
    }

    #[derive(Clone)]
    enum StubMode {
        SuccessWithoutCheckpoint,
        SuccessWithCheckpoint,
        RunFailure,
        CheckpointFailure,
    }

    #[derive(Clone)]
    struct StubExecutor {
        mode: StubMode,
    }

    impl ScheduledTaskExecutor for StubExecutor {
        type Payload = StubPayload;
        type Output = String;
        type Error = StubError;

        fn task_type(&self) -> &'static str {
            "stub.task"
        }

        fn parse_error(&self, error: serde_json::Error) -> Self::Error {
            StubError {
                message: format!("invalid payload: {error}"),
                retryable: false,
                retry_delay_seconds: None,
            }
        }

        fn run(
            &self,
            _task: ScheduledTask,
            payload: Self::Payload,
        ) -> BoxFuture<Result<Self::Output, Self::Error>> {
            let mode = self.mode.clone();
            Box::pin(async move {
                match mode {
                    StubMode::SuccessWithoutCheckpoint | StubMode::SuccessWithCheckpoint => {
                        Ok(payload.value)
                    }
                    StubMode::RunFailure => Err(StubError {
                        message: "run failed".to_string(),
                        retryable: true,
                        retry_delay_seconds: Some(15),
                    }),
                    StubMode::CheckpointFailure => Ok(payload.value),
                }
            })
        }

        fn completed_checkpoint(
            &self,
            _task_id: &str,
            output: &Self::Output,
        ) -> Result<Option<serde_json::Value>, Self::Error> {
            match self.mode {
                StubMode::SuccessWithoutCheckpoint => Ok(None),
                StubMode::SuccessWithCheckpoint => Ok(Some(serde_json::json!({ "value": output }))),
                StubMode::CheckpointFailure => Err(StubError {
                    message: "checkpoint failed".to_string(),
                    retryable: false,
                    retry_delay_seconds: None,
                }),
                StubMode::RunFailure => unreachable!("run failure never builds checkpoint"),
            }
        }

        fn failed_outcome(&self, error: Self::Error) -> TaskRunOutcome {
            TaskRunOutcome::Failed {
                checkpoint: None,
                error_message: error.to_string(),
                retryable: error.retryable,
                retry_delay_seconds: error.retry_delay_seconds,
            }
        }
    }

    fn task(payload: serde_json::Value) -> ScheduledTask {
        ScheduledTask::new(
            NewTask {
                id: "task-1".to_string(),
                user_id: "user-1".to_string(),
                task_type: "stub.task".to_string(),
                payload,
                retry_strategy: RetryStrategy::Never,
                dedupe_key: "dedupe-1".to_string(),
                execution_timeout_seconds: 10,
                leader_only: false,
            },
            1,
        )
        .expect("task should build")
    }

    #[tokio::test]
    async fn invalid_payload_returns_non_retryable_failed_outcome() {
        let handler = scheduled_task_handler(StubExecutor {
            mode: StubMode::SuccessWithCheckpoint,
        });

        let outcome = handler.run(task(serde_json::json!({}))).await;

        assert!(matches!(
            outcome,
            TaskRunOutcome::Failed {
                checkpoint: None,
                retryable: false,
                retry_delay_seconds: None,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn success_without_checkpoint_returns_completed_without_checkpoint() {
        let handler = scheduled_task_handler(StubExecutor {
            mode: StubMode::SuccessWithoutCheckpoint,
        });

        let outcome = handler
            .run(task(serde_json::json!({ "value": "ok" })))
            .await;

        assert!(matches!(
            outcome,
            TaskRunOutcome::Completed { checkpoint: None }
        ));
    }

    #[tokio::test]
    async fn success_with_checkpoint_returns_completed_with_checkpoint() {
        let handler = scheduled_task_handler(StubExecutor {
            mode: StubMode::SuccessWithCheckpoint,
        });

        let outcome = handler
            .run(task(serde_json::json!({ "value": "ok" })))
            .await;

        assert_eq!(
            match outcome {
                TaskRunOutcome::Completed { checkpoint } => checkpoint,
                TaskRunOutcome::Failed { .. } => None,
            },
            Some(serde_json::json!({ "value": "ok" }))
        );
    }

    #[tokio::test]
    async fn run_failure_preserves_retryability_and_retry_delay() {
        let handler = scheduled_task_handler(StubExecutor {
            mode: StubMode::RunFailure,
        });

        let outcome = handler
            .run(task(serde_json::json!({ "value": "ok" })))
            .await;

        assert!(matches!(
            outcome,
            TaskRunOutcome::Failed {
                error_message,
                retryable: true,
                retry_delay_seconds: Some(15),
                ..
            } if error_message == "run failed"
        ));
    }

    #[tokio::test]
    async fn checkpoint_failure_maps_to_failed_outcome() {
        let handler = scheduled_task_handler(StubExecutor {
            mode: StubMode::CheckpointFailure,
        });

        let outcome = handler
            .run(task(serde_json::json!({ "value": "ok" })))
            .await;

        assert!(matches!(
            outcome,
            TaskRunOutcome::Failed {
                error_message,
                retryable: false,
                retry_delay_seconds: None,
                ..
            } if error_message == "checkpoint failed"
        ));
    }

    #[test]
    fn build_scheduled_task_serializes_payload_and_builds_task() {
        let task = build_scheduled_task(NewScheduledTaskInput {
            id: "task-1".to_string(),
            user_id: "user-1".to_string(),
            task_type: "stub.task",
            payload: StubPayload {
                value: "ok".to_string(),
            },
            retry_strategy: RetryStrategy::Never,
            dedupe_key: "dedupe-1".to_string(),
            execution_timeout_seconds: 30,
            leader_only: false,
            now_epoch_seconds: 1,
        })
        .expect("task should build");

        assert_eq!(task.task_type, "stub.task");
        assert_eq!(task.payload, serde_json::json!({ "value": "ok" }));
    }

    #[test]
    fn build_scheduled_task_returns_scheduler_validation_errors() {
        let error = build_scheduled_task(NewScheduledTaskInput {
            id: "".to_string(),
            user_id: "user-1".to_string(),
            task_type: "stub.task",
            payload: StubPayload {
                value: "ok".to_string(),
            },
            retry_strategy: RetryStrategy::Never,
            dedupe_key: "dedupe-1".to_string(),
            execution_timeout_seconds: 30,
            leader_only: false,
            now_epoch_seconds: 1,
        })
        .expect_err("empty id should fail");

        assert!(matches!(
            error,
            BuildScheduledTaskError::Scheduler(TaskSchedulerError::Validation(message))
                if message == "task id is required"
        ));
    }

    #[test]
    fn parse_optional_json_value_returns_none_when_value_missing() {
        let parsed = parse_optional_json_value::<StubCheckpoint, StubError>(
            None,
            "invalid checkpoint",
            stub_error,
        )
        .expect("missing optional value should succeed");

        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_optional_json_value_parses_value_when_present() {
        let parsed = parse_optional_json_value::<StubCheckpoint, StubError>(
            Some(json!({ "value": "ok" })),
            "invalid checkpoint",
            stub_error,
        )
        .expect("valid value should parse");

        assert_eq!(
            parsed,
            Some(StubCheckpoint {
                value: "ok".to_string(),
            })
        );
    }

    #[test]
    fn parse_optional_json_value_maps_invalid_json_error() {
        let error = parse_optional_json_value::<StubCheckpoint, StubError>(
            Some(json!({ "value": 5 })),
            "invalid checkpoint",
            stub_error,
        )
        .expect_err("invalid value should fail");

        assert!(error.message.starts_with("invalid checkpoint: "));
    }

    #[test]
    fn parse_required_json_value_returns_missing_error_when_absent() {
        let error = parse_required_json_value::<StubCheckpoint, StubError>(
            None,
            "missing checkpoint",
            "invalid checkpoint",
            stub_error,
        )
        .expect_err("missing required value should fail");

        assert_eq!(error.message, "missing checkpoint");
    }

    #[test]
    fn parse_required_json_value_maps_invalid_json_error() {
        let error = parse_required_json_value::<StubCheckpoint, StubError>(
            Some(json!({ "value": 5 })),
            "missing checkpoint",
            "invalid checkpoint",
            stub_error,
        )
        .expect_err("invalid required value should fail");

        assert!(error.message.starts_with("invalid checkpoint: "));
    }

    #[test]
    fn parse_failed_or_error_message_prefers_parsed_error() {
        let error = parse_failed_or_error_message::<StubError>(
            Some(StubError {
                message: "typed failure".to_string(),
                retryable: false,
                retry_delay_seconds: None,
            }),
            Some("raw failure".to_string()),
            "missing failure",
            stub_error,
        );

        assert_eq!(error.message, "typed failure");
    }

    #[test]
    fn parse_failed_or_error_message_falls_back_to_error_message() {
        let error = parse_failed_or_error_message::<StubError>(
            None,
            Some("raw failure".to_string()),
            "missing failure",
            stub_error,
        );

        assert_eq!(error.message, "raw failure");
    }

    #[test]
    fn parse_failed_or_error_message_returns_missing_message_when_empty() {
        let error =
            parse_failed_or_error_message::<StubError>(None, None, "missing failure", stub_error);

        assert_eq!(error.message, "missing failure");
    }

    #[test]
    fn serialize_json_value_serializes_struct() {
        let value = serialize_json_value(
            &StubCheckpoint {
                value: "ok".to_string(),
            },
            "serialize failed",
            stub_error,
        )
        .expect("serializable checkpoint should succeed");

        assert_eq!(value, json!({ "value": "ok" }));
    }

    fn stub_error(message: String) -> StubError {
        StubError {
            message,
            retryable: false,
            retry_delay_seconds: None,
        }
    }
}
