use std::fmt;

#[derive(Clone, Debug)]
pub enum AdminPromptPreviewError {
    InvalidDate,
    FutureDate,
    NoCompletedWorkoutForDate,
    Settings(String),
    Repository(String),
    TargetResolution(String),
    Llm(crate::domain::llm::LlmError),
}

impl fmt::Display for AdminPromptPreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDate => write!(f, "date must be YYYY-MM-DD"),
            Self::FutureDate => write!(f, "date cannot be in the future"),
            Self::NoCompletedWorkoutForDate => {
                write!(f, "no completed workout found for date")
            }
            Self::Settings(message) => write!(f, "{message}"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::TargetResolution(message) => write!(f, "{message}"),
            Self::Llm(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AdminPromptPreviewError {}
