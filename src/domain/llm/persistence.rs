use std::time::Duration;

use tracing::{info, warn};

use super::{merge_provider_transcript_entries, BoxFuture, LlmChatMessage};

pub struct RetryConfig {
    pub max_attempts: usize,
    pub backoff_base_ms: u64,
}

pub struct RetryContext {
    pub write_label: &'static str,
    pub user_message_id: String,
    pub scope_label: &'static str,
    pub scope_value: String,
    pub operation_status: Option<String>,
}

pub struct ProviderTranscriptSnapshot<T> {
    pub latest_state: T,
    pub provider_transcript: Vec<LlmChatMessage>,
}

pub async fn retry_persist<T, E>(
    config: RetryConfig,
    is_retryable: impl Fn(&E) -> bool,
    mut operation: impl FnMut() -> BoxFuture<Result<T, E>>,
    ctx: &RetryContext,
) -> Result<T, E>
where
    E: std::fmt::Display,
{
    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(value) => {
                if attempt > 1 {
                    info!(
                        scope = %ctx.scope_value,
                        scope_label = ctx.scope_label,
                        user_message_id = %ctx.user_message_id,
                        attempt,
                        max_attempts = config.max_attempts,
                        write_label = ctx.write_label,
                        "recovered write after retry"
                    );
                }
                return Ok(value);
            }
            Err(error) if is_retryable(&error) => {
                if attempt == config.max_attempts {
                    return Err(error);
                }

                warn!(
                    scope = %ctx.scope_value,
                    scope_label = ctx.scope_label,
                    user_message_id = %ctx.user_message_id,
                    attempt,
                    max_attempts = config.max_attempts,
                    write_label = ctx.write_label,
                    operation_status = ctx.operation_status,
                    error = %error,
                    "retrying write after repository error"
                );
                tokio::time::sleep(Duration::from_millis(
                    config.backoff_base_ms * attempt as u64,
                ))
                .await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("retry loop exhausted with max_attempts >= 1")
}

pub async fn merge_provider_transcript_with_retry<T, S, E, LoadLatest, PersistMerged>(
    config: RetryConfig,
    is_retryable: impl Fn(&E) -> bool,
    load_latest: LoadLatest,
    persist_merged: PersistMerged,
    pending_provider_transcript: &[LlmChatMessage],
    ctx: &RetryContext,
) -> Result<T, E>
where
    E: std::fmt::Display,
    LoadLatest: FnMut() -> BoxFuture<Result<ProviderTranscriptSnapshot<S>, E>>,
    PersistMerged: FnMut(S, Vec<LlmChatMessage>) -> BoxFuture<Result<T, E>>,
{
    let mut load_latest = load_latest;
    let mut persist_merged = persist_merged;

    for attempt in 1..=config.max_attempts {
        let ProviderTranscriptSnapshot {
            latest_state,
            provider_transcript,
        } = match load_latest().await {
            Ok(snapshot) => snapshot,
            Err(error) if is_retryable(&error) => {
                if attempt == config.max_attempts {
                    return Err(error);
                }

                warn!(
                    scope = %ctx.scope_value,
                    scope_label = ctx.scope_label,
                    user_message_id = %ctx.user_message_id,
                    attempt,
                    max_attempts = config.max_attempts,
                    write_label = ctx.write_label,
                    operation_status = ctx.operation_status,
                    error = %error,
                    "retrying write after repository error"
                );
                tokio::time::sleep(Duration::from_millis(
                    config.backoff_base_ms * attempt as u64,
                ))
                .await;
                continue;
            }
            Err(error) => return Err(error),
        };
        let merged =
            merge_provider_transcript_entries(provider_transcript, pending_provider_transcript);

        match persist_merged(latest_state, merged).await {
            Ok(value) => {
                if attempt > 1 {
                    info!(
                        scope = %ctx.scope_value,
                        scope_label = ctx.scope_label,
                        user_message_id = %ctx.user_message_id,
                        attempt,
                        max_attempts = config.max_attempts,
                        write_label = ctx.write_label,
                        "recovered write after retry"
                    );
                }
                return Ok(value);
            }
            Err(error) if is_retryable(&error) => {
                if attempt == config.max_attempts {
                    return Err(error);
                }

                warn!(
                    scope = %ctx.scope_value,
                    scope_label = ctx.scope_label,
                    user_message_id = %ctx.user_message_id,
                    attempt,
                    max_attempts = config.max_attempts,
                    write_label = ctx.write_label,
                    operation_status = ctx.operation_status,
                    error = %error,
                    "retrying write after repository error"
                );
                tokio::time::sleep(Duration::from_millis(
                    config.backoff_base_ms * attempt as u64,
                ))
                .await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("retry loop exhausted with max_attempts >= 1")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::llm::LlmChatMessage;

    use super::{
        merge_provider_transcript_with_retry, ProviderTranscriptSnapshot, RetryConfig, RetryContext,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestError {
        Retryable(&'static str),
        NonRetryable(&'static str),
    }

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Retryable(message) | Self::NonRetryable(message) => write!(f, "{message}"),
            }
        }
    }

    #[tokio::test]
    async fn merge_provider_transcript_with_retry_reloads_latest_state_after_retryable_conflict() {
        let state = Arc::new(Mutex::new(vec![LlmChatMessage::assistant(
            "persisted assistant",
        )]));
        let load_count = Arc::new(Mutex::new(0usize));
        let persist_count = Arc::new(Mutex::new(0usize));
        let persisted_attempts = Arc::new(Mutex::new(Vec::new()));
        let load_state = state.clone();
        let persist_state = state.clone();
        let load_count_for_loader = load_count.clone();
        let persist_count_for_persist = persist_count.clone();
        let persisted_attempts_for_persist = persisted_attempts.clone();
        let pending = vec![LlmChatMessage::tool("tool-1", "tool output")];
        let ctx = RetryContext {
            write_label: "merge_provider_transcript_with_retry_test",
            user_message_id: "message-1".to_string(),
            scope_label: "scope_id",
            scope_value: "scope-1".to_string(),
            operation_status: None,
        };

        let result = merge_provider_transcript_with_retry(
            RetryConfig {
                max_attempts: 2,
                backoff_base_ms: 0,
            },
            |error| matches!(error, TestError::Retryable(_)),
            move || {
                let state = load_state.clone();
                let load_count = load_count_for_loader.clone();
                Box::pin(async move {
                    let mut calls = load_count.lock().expect("load count lock should succeed");
                    *calls += 1;
                    Ok(ProviderTranscriptSnapshot {
                        latest_state: (),
                        provider_transcript: state
                            .lock()
                            .expect("state lock should succeed")
                            .clone(),
                    })
                })
            },
            move |(), merged| {
                let state = persist_state.clone();
                let persist_count = persist_count_for_persist.clone();
                let persisted_attempts = persisted_attempts_for_persist.clone();
                Box::pin(async move {
                    persisted_attempts
                        .lock()
                        .expect("persisted attempts lock should succeed")
                        .push(merged.clone());
                    let mut persist_calls = persist_count
                        .lock()
                        .expect("persist count lock should succeed");
                    let mut latest = state.lock().expect("state lock should succeed");
                    if *persist_calls == 0 {
                        *persist_calls += 1;
                        *latest = vec![
                            LlmChatMessage::assistant("persisted assistant"),
                            LlmChatMessage::assistant("other writer message"),
                        ];
                        return Err(TestError::Retryable("compare-and-set conflict"));
                    }
                    *latest = merged;
                    Ok(())
                })
            },
            &pending,
            &ctx,
        )
        .await;

        assert_eq!(result, Ok(()));
        assert_eq!(
            *load_count.lock().expect("load count lock should succeed"),
            2
        );
        assert_eq!(
            persisted_attempts
                .lock()
                .expect("persisted attempts lock should succeed")
                .clone(),
            vec![
                vec![
                    LlmChatMessage::assistant("persisted assistant"),
                    LlmChatMessage::tool("tool-1", "tool output"),
                ],
                vec![
                    LlmChatMessage::assistant("persisted assistant"),
                    LlmChatMessage::assistant("other writer message"),
                    LlmChatMessage::tool("tool-1", "tool output"),
                ],
            ]
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").clone(),
            vec![
                LlmChatMessage::assistant("persisted assistant"),
                LlmChatMessage::assistant("other writer message"),
                LlmChatMessage::tool("tool-1", "tool output"),
            ]
        );
    }

    #[tokio::test]
    async fn merge_provider_transcript_with_retry_does_not_retry_non_retryable_error() {
        let load_count = Arc::new(Mutex::new(0usize));
        let persist_count = Arc::new(Mutex::new(0usize));
        let load_count_for_loader = load_count.clone();
        let persist_count_for_persist = persist_count.clone();
        let ctx = RetryContext {
            write_label: "merge_provider_transcript_with_retry_test",
            user_message_id: "message-1".to_string(),
            scope_label: "scope_id",
            scope_value: "scope-1".to_string(),
            operation_status: None,
        };

        let result: Result<(), TestError> = merge_provider_transcript_with_retry(
            RetryConfig {
                max_attempts: 3,
                backoff_base_ms: 0,
            },
            |error| matches!(error, TestError::Retryable(_)),
            move || {
                let load_count = load_count_for_loader.clone();
                Box::pin(async move {
                    let mut calls = load_count.lock().expect("load count lock should succeed");
                    *calls += 1;
                    Ok(ProviderTranscriptSnapshot {
                        latest_state: (),
                        provider_transcript: vec![LlmChatMessage::assistant("persisted assistant")],
                    })
                })
            },
            move |_, _| {
                let persist_count = persist_count_for_persist.clone();
                Box::pin(async move {
                    let mut calls = persist_count
                        .lock()
                        .expect("persist count lock should succeed");
                    *calls += 1;
                    Err(TestError::NonRetryable("permanent failure"))
                })
            },
            &[LlmChatMessage::assistant("pending assistant")],
            &ctx,
        )
        .await;

        assert_eq!(result, Err(TestError::NonRetryable("permanent failure")));
        assert_eq!(
            *load_count.lock().expect("load count lock should succeed"),
            1
        );
        assert_eq!(
            *persist_count
                .lock()
                .expect("persist count lock should succeed"),
            1
        );
    }

    #[tokio::test]
    async fn merge_provider_transcript_with_retry_retries_retryable_load_error() {
        let load_count = Arc::new(Mutex::new(0usize));
        let persist_count = Arc::new(Mutex::new(0usize));
        let load_count_for_loader = load_count.clone();
        let persist_count_for_persist = persist_count.clone();
        let ctx = RetryContext {
            write_label: "merge_provider_transcript_with_retry_test",
            user_message_id: "message-1".to_string(),
            scope_label: "scope_id",
            scope_value: "scope-1".to_string(),
            operation_status: None,
        };

        let result: Result<(), TestError> = merge_provider_transcript_with_retry(
            RetryConfig {
                max_attempts: 2,
                backoff_base_ms: 0,
            },
            |error| matches!(error, TestError::Retryable(_)),
            move || {
                let load_count = load_count_for_loader.clone();
                Box::pin(async move {
                    let mut calls = load_count.lock().expect("load count lock should succeed");
                    *calls += 1;
                    if *calls == 1 {
                        return Err(TestError::Retryable("transient load failure"));
                    }
                    Ok(ProviderTranscriptSnapshot {
                        latest_state: (),
                        provider_transcript: vec![LlmChatMessage::assistant("persisted assistant")],
                    })
                })
            },
            move |_, _| {
                let persist_count = persist_count_for_persist.clone();
                Box::pin(async move {
                    let mut calls = persist_count
                        .lock()
                        .expect("persist count lock should succeed");
                    *calls += 1;
                    Ok(())
                })
            },
            &[LlmChatMessage::assistant("pending assistant")],
            &ctx,
        )
        .await;

        assert_eq!(result, Ok(()));
        assert_eq!(
            *load_count.lock().expect("load count lock should succeed"),
            2
        );
        assert_eq!(
            *persist_count
                .lock()
                .expect("persist count lock should succeed"),
            1
        );
    }
}
