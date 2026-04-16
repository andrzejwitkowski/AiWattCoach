#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedWorkoutToken {
    pub user_id: String,
    pub planned_workout_id: String,
    pub match_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedWorkoutTokenError {
    Repository(String),
}

impl std::fmt::Display for PlannedWorkoutTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PlannedWorkoutTokenError {}

impl PlannedWorkoutToken {
    pub fn new(user_id: String, planned_workout_id: String, match_token: String) -> Self {
        Self {
            user_id,
            planned_workout_id,
            match_token,
        }
    }
}
