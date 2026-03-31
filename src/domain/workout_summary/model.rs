#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkoutSummaryError {
    AlreadyExists,
    NotFound,
    Repository(String),
    Validation(String),
}

impl std::fmt::Display for WorkoutSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists => write!(f, "workout summary already exists"),
            Self::NotFound => write!(f, "workout summary not found"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WorkoutSummaryError {}

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
pub struct WorkoutSummary {
    pub id: String,
    pub user_id: String,
    pub event_id: String,
    pub rpe: Option<u8>,
    pub messages: Vec<ConversationMessage>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessageResult {
    pub summary: WorkoutSummary,
    pub user_message: ConversationMessage,
    pub coach_message: ConversationMessage,
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

impl WorkoutSummary {
    pub fn new(id: String, user_id: String, event_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            id,
            user_id,
            event_id,
            rpe: None,
            messages: Vec::new(),
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}

pub fn validate_rpe(rpe: u8) -> Result<u8, WorkoutSummaryError> {
    match rpe {
        1..=10 => Ok(rpe),
        _ => Err(WorkoutSummaryError::Validation(
            "rpe must be between 1 and 10".to_string(),
        )),
    }
}

pub fn validate_message_content(content: &str) -> Result<String, WorkoutSummaryError> {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err(WorkoutSummaryError::Validation(
            "message content must not be empty".to_string(),
        ));
    }

    Ok(trimmed.to_string())
}
