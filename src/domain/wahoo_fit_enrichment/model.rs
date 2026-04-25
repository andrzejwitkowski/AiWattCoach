use serde::{Deserialize, Serialize};

use crate::domain::{
    completed_workouts::{CompletedWorkoutDetails, CompletedWorkoutMetrics},
    wahoo::WahooError,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedWahooFitWorkout {
    pub duration_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
    pub activity_type: Option<String>,
    pub trainer: Option<bool>,
    pub metrics: CompletedWorkoutMetrics,
    pub details: CompletedWorkoutDetails,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WahooFitEnrichmentTaskPayload {
    pub user_id: String,
    pub completed_workout_id: String,
    pub wahoo_workout_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooFitEnrichmentError {
    NotFound,
    DownloadUnavailable(String),
    Parse(String),
    Wahoo(WahooError),
    CompletedWorkoutRepository(String),
    FitFileRepository(String),
    Scheduler(String),
    TrainingLoad(String),
}

impl std::fmt::Display for WahooFitEnrichmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "completed workout not found"),
            Self::DownloadUnavailable(message)
            | Self::Parse(message)
            | Self::CompletedWorkoutRepository(message)
            | Self::FitFileRepository(message)
            | Self::Scheduler(message)
            | Self::TrainingLoad(message) => write!(f, "{message}"),
            Self::Wahoo(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for WahooFitEnrichmentError {}
