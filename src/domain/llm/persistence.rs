use std::time::Duration;

use tracing::{info, warn};

use super::BoxFuture;

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
