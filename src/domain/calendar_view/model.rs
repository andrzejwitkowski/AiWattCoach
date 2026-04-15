#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEntryViewError {
    Repository(String),
}

impl std::fmt::Display for CalendarEntryViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CalendarEntryViewError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEntryKind {
    PlannedWorkout,
    CompletedWorkout,
    Race,
    SpecialDay,
}

impl CalendarEntryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlannedWorkout => "planned_workout",
            Self::CompletedWorkout => "completed_workout",
            Self::Race => "race",
            Self::SpecialDay => "special_day",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEntrySummary {
    pub training_stress_score: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub normalized_power_watts: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEntrySync {
    pub linked_intervals_event_id: Option<i64>,
    pub sync_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEntryRace {
    pub distance_meters: i32,
    pub discipline: String,
    pub priority: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEntryView {
    pub entry_id: String,
    pub user_id: String,
    pub entry_kind: CalendarEntryKind,
    pub date: String,
    pub start_date_local: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub raw_workout_doc: Option<String>,
    pub planned_workout_id: Option<String>,
    pub completed_workout_id: Option<String>,
    pub race_id: Option<String>,
    pub special_day_id: Option<String>,
    pub race: Option<CalendarEntryRace>,
    pub summary: Option<CalendarEntrySummary>,
    pub sync: Option<CalendarEntrySync>,
}
