use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub enum IntervalsError {
    Unauthenticated,
    CredentialsNotConfigured,
    ApiError(String),
    ConnectionError(String),
    NotFound,
    Internal(String),
}

impl std::fmt::Display for IntervalsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::CredentialsNotConfigured => {
                write!(f, "Intervals.icu credentials are not configured")
            }
            Self::ApiError(message) => write!(f, "Intervals.icu API error: {message}"),
            Self::ConnectionError(message) => write!(f, "Connection error: {message}"),
            Self::NotFound => write!(f, "Event not found"),
            Self::Internal(message) => write!(f, "Internal error: {message}"),
        }
    }
}

impl std::error::Error for IntervalsError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalsCredentials {
    pub api_key: String,
    pub athlete_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventCategory {
    Workout,
    Race,
    Note,
    Target,
    Season,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: EventCategory,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateEvent {
    pub category: EventCategory,
    pub start_date_local: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateEvent {
    pub category: Option<EventCategory>,
    pub start_date_local: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateRange {
    pub oldest: String,
    pub newest: String,
}
