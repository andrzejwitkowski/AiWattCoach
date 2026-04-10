use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarLabelError {
    Unauthenticated,
    Validation(String),
    Unavailable(String),
    Internal(String),
}

impl std::fmt::Display for CalendarLabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::Validation(message) => write!(f, "{message}"),
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CalendarLabelError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarLabelsResponse {
    pub labels_by_date: BTreeMap<String, BTreeMap<String, CalendarLabel>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarLabel {
    pub label_key: String,
    pub date: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub payload: CalendarLabelPayload,
}

impl CalendarLabel {
    pub fn kind(&self) -> &'static str {
        self.payload.kind()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarLabelPayload {
    Race(CalendarRaceLabel),
    Activity(CalendarActivityLabel),
    Health(CalendarHealthLabel),
    Custom(CalendarCustomLabel),
}

impl CalendarLabelPayload {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Race(_) => "race",
            Self::Activity(_) => "activity",
            Self::Health(_) => "health",
            Self::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarRaceLabel {
    pub race_id: String,
    pub date: String,
    pub name: String,
    pub distance_meters: i32,
    /// Discipline string as stored in the races domain (e.g. "road", "mtb", "gravel", "cyclocross").
    pub discipline: String,
    /// Priority string as stored in the races domain (e.g. "A", "B", "C").
    pub priority: String,
    /// Sync status string as stored in the races domain (e.g. "pending", "synced", "failed", "pending_delete").
    pub sync_status: String,
    pub linked_intervals_event_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarActivityLabel {
    pub label_id: String,
    pub activity_kind: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarHealthLabel {
    pub label_id: String,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarCustomLabel {
    pub label_id: String,
    pub value: String,
}
