#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedCompletedWorkoutLinkMatchSource {
    Explicit,
    Token,
    Heuristic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedCompletedWorkoutLink {
    pub user_id: String,
    pub planned_workout_id: String,
    pub completed_workout_id: String,
    pub match_source: PlannedCompletedWorkoutLinkMatchSource,
    pub matched_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedCompletedWorkoutLinkError {
    Repository(String),
}

impl std::fmt::Display for PlannedCompletedWorkoutLinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PlannedCompletedWorkoutLinkError {}

impl PlannedCompletedWorkoutLink {
    pub fn new(
        user_id: String,
        planned_workout_id: String,
        completed_workout_id: String,
        match_source: PlannedCompletedWorkoutLinkMatchSource,
        matched_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            planned_workout_id,
            completed_workout_id,
            match_source,
            matched_at_epoch_seconds,
        }
    }
}
