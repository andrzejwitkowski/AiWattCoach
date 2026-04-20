use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TaskDocument {
    #[serde(rename = "_id")]
    pub(super) id: String,
    pub(super) user_id: String,
    pub(super) task_type: String,
    pub(super) status: String,
    pub(super) payload: serde_json::Value,
    pub(super) checkpoint: Option<serde_json::Value>,
    pub(super) retry_strategy: RetryStrategyDocument,
    pub(super) dedupe_key: String,
    pub(super) error_message: Option<String>,
    pub(super) attempt_count: i64,
    pub(super) next_attempt_at_epoch_seconds: i64,
    pub(super) claimed_by: Option<String>,
    pub(super) lease_expires_at_epoch_seconds: Option<i64>,
    pub(super) last_heartbeat_at_epoch_seconds: Option<i64>,
    pub(super) execution_timeout_seconds: i64,
    pub(super) timed_out_at_epoch_seconds: Option<i64>,
    pub(super) leader_only: bool,
    pub(super) created_at_epoch_seconds: i64,
    pub(super) updated_at_epoch_seconds: i64,
    pub(super) started_at_epoch_seconds: Option<i64>,
    pub(super) finished_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RetryStrategyDocument {
    pub(super) kind: String,
    pub(super) max_attempts: Option<i64>,
    pub(super) delay_seconds: Option<i64>,
    pub(super) initial_delay_seconds: Option<i64>,
    pub(super) max_delay_seconds: Option<i64>,
}
