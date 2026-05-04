use mongodb::bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

use crate::domain::{llm::LlmChatMessage, workout_summary::PublicToolCall};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct WorkoutSummaryDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<ObjectId>,
    pub(super) summary_id: String,
    pub(super) user_id: String,
    #[serde(alias = "event_id")]
    pub(super) workout_id: String,
    pub(super) rpe: Option<i32>,
    pub(super) messages: Vec<ConversationMessageDocument>,
    #[serde(default)]
    pub(super) hidden_transcript: Vec<LlmChatMessage>,
    pub(super) saved_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) saved_at: Option<DateTime>,
    #[serde(default)]
    pub(super) workout_recap_text: Option<String>,
    #[serde(default)]
    pub(super) workout_recap_provider: Option<String>,
    #[serde(default)]
    pub(super) workout_recap_model: Option<String>,
    #[serde(default)]
    pub(super) workout_recap_generated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) workout_recap_generated_at: Option<DateTime>,
    pub(super) created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) created_at: Option<DateTime>,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) updated_at: Option<DateTime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ConversationMessageDocument {
    pub(super) id: String,
    pub(super) role: String,
    pub(super) content: String,
    #[serde(default)]
    pub(super) tool_call: Option<PublicToolCall>,
    pub(super) created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) created_at: Option<DateTime>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct WorkoutSummaryMessageLookupDocument {
    #[serde(default)]
    pub(super) messages: Vec<ConversationMessageDocument>,
}
