use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
pub enum IntervalsError {
    Unauthenticated,
    CredentialsNotConfigured,
    ApiError(String),
    ConnectionError(String),
    NotFound,
    Internal(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFileUpload {
    pub filename: String,
    pub file_contents: Option<String>,
    pub file_contents_base64: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadActivity {
    pub filename: String,
    pub file_bytes: Vec<u8>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub device_name: Option<String>,
    pub external_id: Option<String>,
    pub paired_event_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UploadedActivities {
    pub created: bool,
    pub activity_ids: Vec<String>,
    pub activities: Vec<Activity>,
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

impl EventCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Workout => "WORKOUT",
            Self::Race => "RACE",
            Self::Note => "NOTE",
            Self::Target => "TARGET",
            Self::Season => "SEASON",
            Self::Other => "OTHER",
        }
    }

    pub fn from_api_str(value: &str) -> Self {
        Self::from_str(value).unwrap_or(Self::Other)
    }
}

impl FromStr for EventCategory {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "WORKOUT" => Ok(Self::Workout),
            "RACE" => Ok(Self::Race),
            "NOTE" => Ok(Self::Note),
            "TARGET" => Ok(Self::Target),
            "SEASON" => Ok(Self::Season),
            "OTHER" => Ok(Self::Other),
            _ => Err(()),
        }
    }
}

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
    pub file_upload: Option<EventFileUpload>,
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
    pub file_upload: Option<EventFileUpload>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateRange {
    pub oldest: String,
    pub newest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityMetrics {
    pub training_stress_score: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub efficiency_factor: Option<f64>,
    pub variability_index: Option<f64>,
    pub average_power_watts: Option<i32>,
    pub ftp_watts: Option<i32>,
    pub total_work_joules: Option<i32>,
    pub calories: Option<i32>,
    pub trimp: Option<f64>,
    pub power_load: Option<i32>,
    pub heart_rate_load: Option<i32>,
    pub pace_load: Option<i32>,
    pub strain_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityZoneTime {
    pub zone_id: String,
    pub seconds: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityInterval {
    pub id: Option<i32>,
    pub label: Option<String>,
    pub interval_type: Option<String>,
    pub group_id: Option<String>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
    pub start_time_seconds: Option<i32>,
    pub end_time_seconds: Option<i32>,
    pub moving_time_seconds: Option<i32>,
    pub elapsed_time_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub training_stress_score: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub average_stride_meters: Option<f64>,
    pub zone: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityIntervalGroup {
    pub id: String,
    pub count: Option<i32>,
    pub start_index: Option<i32>,
    pub moving_time_seconds: Option<i32>,
    pub elapsed_time_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub training_stress_score: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub average_stride_meters: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityStream {
    pub stream_type: String,
    pub name: Option<String>,
    pub data: Option<serde_json::Value>,
    pub data2: Option<serde_json::Value>,
    pub value_type_is_array: bool,
    pub custom: bool,
    pub all_null: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityDetails {
    pub intervals: Vec<ActivityInterval>,
    pub interval_groups: Vec<ActivityIntervalGroup>,
    pub streams: Vec<ActivityStream>,
    pub interval_summary: Vec<String>,
    pub skyline_chart: Vec<String>,
    pub power_zone_times: Vec<ActivityZoneTime>,
    pub heart_rate_zone_times: Vec<i32>,
    pub pace_zone_times: Vec<i32>,
    pub gap_zone_times: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    pub athlete_id: Option<String>,
    pub start_date_local: String,
    pub start_date: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub activity_type: Option<String>,
    pub source: Option<String>,
    pub external_id: Option<String>,
    pub device_name: Option<String>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub elapsed_time_seconds: Option<i32>,
    pub total_elevation_gain_meters: Option<f64>,
    pub total_elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub trainer: bool,
    pub commute: bool,
    pub race: bool,
    pub has_heart_rate: bool,
    pub stream_types: Vec<String>,
    pub tags: Vec<String>,
    pub metrics: ActivityMetrics,
    pub details: ActivityDetails,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct UpdateActivity {
    pub name: Option<String>,
    pub description: Option<String>,
    pub activity_type: Option<String>,
    pub trainer: Option<bool>,
    pub commute: Option<bool>,
    pub race: Option<bool>,
}
