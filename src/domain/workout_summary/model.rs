use crate::domain::llm::{LlmCacheUsage, LlmError, LlmProvider, LlmTokenUsage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkoutSummaryError {
    AlreadyExists,
    Locked,
    NotFound,
    ReplyAlreadyPending,
    Repository(String),
    Llm(LlmError),
    Validation(String),
}

impl std::fmt::Display for WorkoutSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists => write!(f, "workout summary already exists"),
            Self::Locked => write!(f, "workout summary is saved and cannot be edited"),
            Self::NotFound => write!(f, "workout summary not found"),
            Self::ReplyAlreadyPending => {
                write!(
                    f,
                    "coach reply generation is already pending for this message"
                )
            }
            Self::Repository(message) => write!(f, "{message}"),
            Self::Llm(error) => write!(f, "{error}"),
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WorkoutSummaryError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachReplyOperationStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachReplyOperation {
    pub user_id: String,
    pub workout_id: String,
    pub user_message_id: String,
    pub status: CoachReplyOperationStatus,
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub provider_request_id: Option<String>,
    pub coach_message_id: Option<String>,
    pub cache_scope_key: Option<String>,
    pub provider_cache_id: Option<String>,
    pub token_usage: Option<LlmTokenUsage>,
    pub cache_usage: Option<LlmCacheUsage>,
    pub error_message: Option<String>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoachReplyClaimResult {
    Claimed(CoachReplyOperation),
    Existing(CoachReplyOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedCoachReply {
    pub provider: LlmProvider,
    pub model: String,
    pub provider_request_id: Option<String>,
    pub coach_message_id: String,
    pub provider_cache_id: Option<String>,
    pub token_usage: LlmTokenUsage,
    pub cache_usage: LlmCacheUsage,
    pub updated_at_epoch_seconds: i64,
}

impl CoachReplyOperation {
    pub fn pending(
        user_id: String,
        workout_id: String,
        user_message_id: String,
        cache_scope_key: Option<String>,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            workout_id,
            user_message_id,
            status: CoachReplyOperationStatus::Pending,
            provider: None,
            model: None,
            provider_request_id: None,
            coach_message_id: None,
            cache_scope_key,
            provider_cache_id: None,
            token_usage: None,
            cache_usage: None,
            error_message: None,
            created_at_epoch_seconds,
            updated_at_epoch_seconds: created_at_epoch_seconds,
        }
    }

    pub fn mark_completed(&self, reply: CompletedCoachReply) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Completed,
            provider: Some(reply.provider),
            model: Some(reply.model),
            provider_request_id: reply.provider_request_id,
            coach_message_id: Some(reply.coach_message_id),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: reply.provider_cache_id,
            token_usage: Some(reply.token_usage),
            cache_usage: Some(reply.cache_usage),
            error_message: None,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: reply.updated_at_epoch_seconds,
        }
    }

    pub fn mark_failed(&self, error_message: String, updated_at_epoch_seconds: i64) -> Self {
        Self {
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            user_message_id: self.user_message_id.clone(),
            status: CoachReplyOperationStatus::Failed,
            provider: self.provider.clone(),
            model: self.model.clone(),
            provider_request_id: self.provider_request_id.clone(),
            coach_message_id: self.coach_message_id.clone(),
            cache_scope_key: self.cache_scope_key.clone(),
            provider_cache_id: self.provider_cache_id.clone(),
            token_usage: self.token_usage.clone(),
            cache_usage: self.cache_usage.clone(),
            error_message: Some(error_message),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkoutSummary {
    pub id: String,
    pub user_id: String,
    pub workout_id: String,
    pub rpe: Option<u8>,
    pub messages: Vec<ConversationMessage>,
    pub saved_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Coach,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedUserMessage {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoachReply {
    pub summary: WorkoutSummary,
    pub coach_message: ConversationMessage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessageResult {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
    pub coach_message: ConversationMessage,
}

impl WorkoutSummary {
    pub fn new(id: String, user_id: String, workout_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            id,
            user_id,
            workout_id,
            rpe: None,
            messages: Vec::new(),
            saved_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}

pub fn validate_rpe(rpe: u8) -> Result<u8, WorkoutSummaryError> {
    if (1..=10).contains(&rpe) {
        Ok(rpe)
    } else {
        Err(WorkoutSummaryError::Validation(
            "rpe must be between 1 and 10".to_string(),
        ))
    }
}

pub fn validate_message_content(content: &str) -> Result<String, WorkoutSummaryError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(WorkoutSummaryError::Validation(
            "message content must not be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 2000 {
        return Err(WorkoutSummaryError::Validation(
            "message must be 2000 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}
