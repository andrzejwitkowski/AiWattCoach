#[derive(Clone, Debug, PartialEq)]
pub struct WorkoutPestParseError {
    message: String,
}

impl WorkoutPestParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WorkoutPestParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for WorkoutPestParseError {}
