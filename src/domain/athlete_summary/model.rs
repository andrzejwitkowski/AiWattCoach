use crate::domain::llm::LlmError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AthleteSummaryError {
    NotConfigured,
    Unavailable(String),
    Repository(String),
    Llm(LlmError),
}

impl std::fmt::Display for AthleteSummaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "athlete summary generation is not configured"),
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::Llm(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AthleteSummaryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AthleteSummary {
    pub user_id: String,
    pub summary_text: String,
    pub generated_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AthleteSummaryGenerationOperationStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AthleteSummaryGenerationOperation {
    pub user_id: String,
    pub status: AthleteSummaryGenerationOperationStatus,
    pub summary_text: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub error_message: Option<String>,
    pub started_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
    pub attempt_count: u32,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AthleteSummaryGenerationClaimResult {
    Claimed(AthleteSummaryGenerationOperation),
    Existing(AthleteSummaryGenerationOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AthleteSummaryState {
    pub summary: Option<AthleteSummary>,
    pub stale: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnsuredAthleteSummary {
    pub summary: AthleteSummary,
    pub was_regenerated: bool,
}
