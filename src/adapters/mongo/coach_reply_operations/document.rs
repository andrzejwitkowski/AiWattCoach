use mongodb::bson::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct CoachReplyOperationDocument {
    pub(super) user_id: String,
    pub(super) workout_id: String,
    pub(super) user_message_id: String,
    pub(super) status: String,
    pub(super) failure_kind: Option<String>,
    pub(super) provider: Option<String>,
    pub(super) model: Option<String>,
    pub(super) provider_request_id: Option<String>,
    pub(super) coach_message_id: Option<String>,
    pub(super) cache_scope_key: Option<String>,
    pub(super) provider_cache_id: Option<String>,
    pub(super) token_usage: Option<crate::domain::llm::LlmTokenUsage>,
    pub(super) cache_usage: Option<crate::domain::llm::LlmCacheUsage>,
    pub(super) response_message: Option<String>,
    pub(super) error_message: Option<String>,
    pub(super) started_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) started_at: Option<DateTime>,
    pub(super) last_attempt_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) last_attempt_at: Option<DateTime>,
    pub(super) attempt_count: i64,
    pub(super) created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) created_at: Option<DateTime>,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) updated_at: Option<DateTime>,
}
