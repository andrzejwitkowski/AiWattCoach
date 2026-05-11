use crate::domain::llm::{
    CompletedLlmReply, LlmChatMessage, LlmError, LlmReplyClaimResult, LlmReplyOperation,
    LlmReplyOperationFailureKind, LlmReplyOperationStatus, PendingLlmReplyCheckpoint,
};
use serde::{Deserialize, Serialize};

use crate::domain::workout_summary::PublicToolCall;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachConversationSurface {
    Calendar,
}

impl CoachConversationSurface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Calendar => "calendar",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachConversationStatus {
    Active,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachConversationFocus {
    Overview,
}

impl CoachConversationFocus {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Overview => "overview",
        }
    }

    pub fn cache_scope_suffix(&self) -> &'static str {
        match self {
            Self::Overview => "overview",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachConversation {
    pub conversation_id: String,
    pub user_id: String,
    pub surface: CoachConversationSurface,
    pub status: CoachConversationStatus,
    pub focus: CoachConversationFocus,
    pub provider_transcript: Vec<LlmChatMessage>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl CoachConversation {
    pub fn new(
        conversation_id: String,
        user_id: String,
        surface: CoachConversationSurface,
        focus: CoachConversationFocus,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            conversation_id,
            user_id,
            surface,
            status: CoachConversationStatus::Active,
            focus,
            provider_transcript: Vec::new(),
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn archive(&self, updated_at_epoch_seconds: i64) -> Self {
        Self {
            conversation_id: self.conversation_id.clone(),
            user_id: self.user_id.clone(),
            surface: self.surface.clone(),
            status: CoachConversationStatus::Archived,
            focus: self.focus.clone(),
            provider_transcript: self.provider_transcript.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoachConversationMessageRole {
    User,
    Coach,
    System,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub user_id: String,
    pub role: CoachConversationMessageRole,
    pub content: String,
    pub tool_call: Option<PublicToolCall>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    pub created_at_epoch_seconds: i64,
}

pub type CoachConversationReplyOperationFailureKind = LlmReplyOperationFailureKind;

pub type CoachConversationReplyOperationStatus = LlmReplyOperationStatus;
pub type CoachConversationReplyOperation = LlmReplyOperation;
pub type CoachConversationReplyClaimResult = LlmReplyClaimResult;
pub type CompletedCoachConversationReply = CompletedLlmReply;
pub type PendingCoachConversationReplyCheckpoint = PendingLlmReplyCheckpoint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoachConversationError {
    NotFound,
    Archived,
    ReplyAlreadyPending,
    Repository(String),
    Llm(LlmError),
    Validation(String),
}

impl std::fmt::Display for CoachConversationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "coach conversation not found"),
            Self::Archived => write!(f, "coach conversation is archived"),
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

impl std::error::Error for CoachConversationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedConversationUserMessage {
    pub conversation: CoachConversation,
    pub messages: Vec<CoachConversationMessage>,
    pub user_message: CoachConversationMessage,
    pub athlete_summary_may_regenerate_before_reply: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoachConversationReply {
    pub conversation: CoachConversation,
    pub messages: Vec<CoachConversationMessage>,
    pub coach_message: CoachConversationMessage,
    pub athlete_summary_was_regenerated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendConversationMessageResult {
    pub conversation: CoachConversation,
    pub messages: Vec<CoachConversationMessage>,
    pub user_message: CoachConversationMessage,
    pub coach_message: CoachConversationMessage,
}

pub fn validate_conversation_message_content(
    content: &str,
) -> Result<String, CoachConversationError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(CoachConversationError::Validation(
            "message content must not be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 2000 {
        return Err(CoachConversationError::Validation(
            "message must be 2000 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}
