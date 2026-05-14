use crate::domain::llm::{
    CompletedLlmReply, LlmChatMessage, LlmError, LlmReplyClaimResult, LlmReplyOperation,
    LlmReplyOperationFailureKind, LlmReplyOperationStatus, PendingLlmReplyCheckpoint,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachQuestion {
    pub id: String,
    pub question: String,
    pub answers: Vec<String>,
    pub free_text_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicToolCall {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
    pub arguments_preview: Option<String>,
}

pub type CoachReplyOperationFailureKind = LlmReplyOperationFailureKind;

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

pub type CoachReplyOperationStatus = LlmReplyOperationStatus;
pub type CoachReplyOperation = LlmReplyOperation;
pub type CoachReplyClaimResult = LlmReplyClaimResult;
pub type CompletedCoachReply = CompletedLlmReply;
pub type PendingCoachReplyCheckpoint = PendingLlmReplyCheckpoint;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkoutSummary {
    pub id: String,
    pub user_id: String,
    pub workout_id: String,
    pub rpe: Option<u8>,
    pub messages: Vec<ConversationMessage>,
    pub provider_transcript: Vec<LlmChatMessage>,
    pub saved_at_epoch_seconds: Option<i64>,
    pub workout_recap_text: Option<String>,
    pub workout_recap_provider: Option<String>,
    pub workout_recap_model: Option<String>,
    pub workout_recap_generated_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkoutRecap {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub generated_at_epoch_seconds: i64,
}

impl WorkoutRecap {
    pub fn generated(
        text: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        generated_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            text: text.into(),
            provider: provider.into(),
            model: model.into(),
            generated_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Coach,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub tool_call: Option<PublicToolCall>,
    pub questions: Vec<CoachQuestion>,
    pub created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedUserMessage {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
    pub athlete_summary_may_regenerate_before_reply: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoachReply {
    pub summary: WorkoutSummary,
    pub coach_message: ConversationMessage,
    pub athlete_summary_was_regenerated: bool,
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
            provider_transcript: Vec::new(),
            saved_at_epoch_seconds: None,
            workout_recap_text: None,
            workout_recap_provider: None,
            workout_recap_model: None,
            workout_recap_generated_at_epoch_seconds: None,
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
