use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::domain::task_scheduler::{
    BoxFuture, ScheduledTask, SharedTaskHandler, TaskHandler, TaskRunOutcome,
};

#[derive(Clone, Debug)]
pub struct TaskFailurePolicy {
    pub retryable: bool,
    pub retry_delay_seconds: Option<i64>,
}

pub trait ScheduledTaskRunner: Send + Sync + 'static {
    type Payload: DeserializeOwned + Send + 'static;
    type Output: Send + 'static;
    type Error: std::error::Error + Send + 'static;

    fn task_type(&self) -> &'static str;

    fn execute(&self, payload: Self::Payload) -> BoxFuture<Result<Self::Output, Self::Error>>;

    fn serialize_checkpoint(&self, output: &Self::Output)
        -> Result<serde_json::Value, Self::Error>;

    fn serialize_error(&self, error: &Self::Error) -> Option<serde_json::Value>;

    fn failure_policy(&self, error: &Self::Error) -> TaskFailurePolicy;
}

struct GenericTaskHandler<R: ScheduledTaskRunner> {
    runner: Arc<R>,
}

impl<R: ScheduledTaskRunner> GenericTaskHandler<R> {
    pub fn new(runner: Arc<R>) -> Self {
        Self { runner }
    }
}

impl<R: ScheduledTaskRunner> TaskHandler for GenericTaskHandler<R> {
    fn task_type(&self) -> &'static str {
        self.runner.task_type()
    }

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let runner = self.runner.clone();
        Box::pin(async move {
            let payload = match serde_json::from_value::<R::Payload>(task.payload.clone()) {
                Ok(payload) => payload,
                Err(error) => {
                    tracing::warn!(
                        task_id = %task.id,
                        task_type = %runner.task_type(),
                        %error,
                        "invalid scheduled task payload"
                    );
                    return TaskRunOutcome::Failed {
                        checkpoint: None,
                        error_message: format!("invalid task payload: {error}"),
                        retryable: false,
                        retry_delay_seconds: None,
                    };
                }
            };

            match runner.execute(payload).await {
                Ok(output) => match runner.serialize_checkpoint(&output) {
                    Ok(checkpoint) => TaskRunOutcome::Completed {
                        checkpoint: Some(checkpoint),
                    },
                    Err(error) => {
                        let policy = runner.failure_policy(&error);
                        let checkpoint = runner.serialize_error(&error);
                        tracing::warn!(
                            task_id = %task.id,
                            task_type = %runner.task_type(),
                            %error,
                            "failed to serialize completed task checkpoint"
                        );
                        TaskRunOutcome::Failed {
                            checkpoint,
                            error_message: error.to_string(),
                            retryable: policy.retryable,
                            retry_delay_seconds: policy.retry_delay_seconds,
                        }
                    }
                },
                Err(error) => {
                    let policy = runner.failure_policy(&error);
                    TaskRunOutcome::Failed {
                        checkpoint: runner.serialize_error(&error),
                        error_message: error.to_string(),
                        retryable: policy.retryable,
                        retry_delay_seconds: policy.retry_delay_seconds,
                    }
                }
            }
        })
    }
}

pub fn scheduled_task_handler<R: ScheduledTaskRunner>(runner: Arc<R>) -> SharedTaskHandler {
    Arc::new(GenericTaskHandler::new(runner))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::domain::task_scheduler::{
        BoxFuture as TaskBoxFuture, NewTask, RetryStrategy, ScheduledTask, TaskRunOutcome,
    };

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestPayload {
        value: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestOutput {
        doubled: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestErrorKind {
        DomainFailure,
        CheckpointFailure,
    }

    #[derive(Debug, Clone)]
    struct TestError(TestErrorKind);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.0 {
                TestErrorKind::DomainFailure => write!(f, "domain failure"),
                TestErrorKind::CheckpointFailure => {
                    write!(f, "checkpoint serialization failure")
                }
            }
        }
    }

    impl std::error::Error for TestError {}

    fn domain_failure() -> TestError {
        TestError(TestErrorKind::DomainFailure)
    }

    fn checkpoint_failure() -> TestError {
        TestError(TestErrorKind::CheckpointFailure)
    }

    #[derive(Clone)]
    struct TestRunner {
        should_succeed: bool,
        checkpoint_serialization_fails: bool,
    }

    impl ScheduledTaskRunner for TestRunner {
        type Payload = TestPayload;
        type Output = TestOutput;
        type Error = TestError;

        fn task_type(&self) -> &'static str {
            "test.task"
        }

        fn execute(
            &self,
            payload: Self::Payload,
        ) -> TaskBoxFuture<Result<Self::Output, Self::Error>> {
            let should_succeed = self.should_succeed;
            Box::pin(async move {
                if should_succeed {
                    Ok(TestOutput {
                        doubled: payload.value * 2,
                    })
                } else {
                    Err(domain_failure())
                }
            })
        }

        fn serialize_checkpoint(
            &self,
            output: &Self::Output,
        ) -> Result<serde_json::Value, Self::Error> {
            if self.checkpoint_serialization_fails {
                return Err(checkpoint_failure());
            }
            serde_json::to_value(output).map_err(|_| checkpoint_failure())
        }

        fn serialize_error(&self, error: &Self::Error) -> Option<serde_json::Value> {
            serde_json::to_value(format!("{error}")).ok()
        }

        fn failure_policy(&self, error: &Self::Error) -> TaskFailurePolicy {
            match error.0 {
                TestErrorKind::DomainFailure => TaskFailurePolicy {
                    retryable: true,
                    retry_delay_seconds: Some(60),
                },
                TestErrorKind::CheckpointFailure => TaskFailurePolicy {
                    retryable: false,
                    retry_delay_seconds: None,
                },
            }
        }
    }

    fn build_test_task() -> ScheduledTask {
        ScheduledTask::new(
            NewTask {
                id: "task-1".to_string(),
                user_id: "user-1".to_string(),
                task_type: "test.task".to_string(),
                payload: serde_json::json!({"value": 21}),
                retry_strategy: RetryStrategy::Fixed {
                    max_attempts: 3,
                    delay_seconds: 30,
                },
                dedupe_key: "test:user-1:task-1".to_string(),
                execution_timeout_seconds: 300,
                leader_only: false,
            },
            1_700_000_000,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn completes_with_checkpoint_on_success() {
        let runner = Arc::new(TestRunner {
            should_succeed: true,
            checkpoint_serialization_fails: false,
        });
        let handler = scheduled_task_handler(runner);

        let outcome = handler.run(build_test_task()).await;

        assert!(
            matches!(&outcome, TaskRunOutcome::Completed { checkpoint } if checkpoint.as_ref().is_some_and(|c| c["doubled"] == serde_json::json!(42))),
            "expected completed with doubled=42, got {outcome:?}"
        );
    }

    #[tokio::test]
    async fn fails_with_policy_on_domain_error() {
        let runner = Arc::new(TestRunner {
            should_succeed: false,
            checkpoint_serialization_fails: false,
        });
        let handler = scheduled_task_handler(runner);

        let outcome = handler.run(build_test_task()).await;

        match outcome {
            TaskRunOutcome::Failed {
                ref error_message,
                retryable,
                retry_delay_seconds,
                ..
            } => {
                assert!(error_message.contains("domain failure"), "{error_message}");
                assert!(retryable);
                assert_eq!(retry_delay_seconds, Some(60));
            }
            other => panic!("expected failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fails_on_checkpoint_serialization_error() {
        let runner = Arc::new(TestRunner {
            should_succeed: true,
            checkpoint_serialization_fails: true,
        });
        let handler = scheduled_task_handler(runner);

        let outcome = handler.run(build_test_task()).await;

        match outcome {
            TaskRunOutcome::Failed {
                ref error_message,
                retryable,
                ..
            } => {
                assert!(
                    error_message.contains("checkpoint serialization"),
                    "{error_message}"
                );
                assert!(!retryable);
            }
            other => panic!("expected failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fails_non_retryable_on_invalid_payload() {
        let runner = Arc::new(TestRunner {
            should_succeed: true,
            checkpoint_serialization_fails: false,
        });
        let handler = scheduled_task_handler(runner);
        let mut task = build_test_task();
        task.payload = serde_json::json!({"bad_field": true});

        let outcome = handler.run(task).await;

        match outcome {
            TaskRunOutcome::Failed {
                ref error_message,
                retryable,
                ref checkpoint,
                ..
            } => {
                assert!(
                    error_message.contains("invalid task payload"),
                    "{error_message}"
                );
                assert!(!retryable);
                assert!(checkpoint.is_none());
            }
            other => panic!("expected failed, got {other:?}"),
        }
    }
}
