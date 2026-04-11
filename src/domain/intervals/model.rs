use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityUploadOperationStatus {
    Pending,
    Failed,
    Uploaded,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityUploadOperation {
    pub operation_key: String,
    pub normalized_external_id: Option<String>,
    pub fallback_identity: Option<String>,
    pub uploaded_activity_ids: Vec<String>,
    pub status: ActivityUploadOperationStatus,
}

impl ActivityUploadOperation {
    pub fn pending(
        operation_key: String,
        normalized_external_id: Option<String>,
        fallback_identity: Option<String>,
    ) -> Self {
        Self {
            operation_key,
            normalized_external_id,
            fallback_identity,
            uploaded_activity_ids: Vec::new(),
            status: ActivityUploadOperationStatus::Pending,
        }
    }

    pub fn mark_uploaded(&self, activity_ids: Vec<String>) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            normalized_external_id: self.normalized_external_id.clone(),
            fallback_identity: self.fallback_identity.clone(),
            uploaded_activity_ids: activity_ids,
            status: ActivityUploadOperationStatus::Uploaded,
        }
    }

    pub fn mark_failed(&self) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            normalized_external_id: self.normalized_external_id.clone(),
            fallback_identity: self.fallback_identity.clone(),
            uploaded_activity_ids: Vec::new(),
            status: ActivityUploadOperationStatus::Failed,
        }
    }

    pub fn mark_completed(&self, activity_ids: Vec<String>) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            normalized_external_id: self.normalized_external_id.clone(),
            fallback_identity: self.fallback_identity.clone(),
            uploaded_activity_ids: activity_ids,
            status: ActivityUploadOperationStatus::Completed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityUploadOperationClaimResult {
    Claimed(ActivityUploadOperation),
    Existing(ActivityUploadOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityDeduplicationIdentity {
    pub normalized_external_id: Option<String>,
    pub fallback_identity: Option<String>,
}

impl ActivityDeduplicationIdentity {
    pub fn from_activity(activity: &Activity) -> Self {
        Self {
            normalized_external_id: normalize_external_id(activity.external_id.as_deref()),
            fallback_identity: ActivityFallbackIdentity::from_activity(activity)
                .map(|identity| identity.as_fingerprint()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityFallbackIdentity {
    pub start_bucket: String,
    pub activity_type_bucket: String,
    pub duration_bucket_seconds: i32,
    pub distance_bucket_meters: Option<i32>,
    pub trainer: bool,
}

impl ActivityFallbackIdentity {
    pub fn from_activity(activity: &Activity) -> Option<Self> {
        let start_bucket = bucket_start_time(
            activity
                .start_date
                .as_deref()
                .unwrap_or(&activity.start_date_local),
        )?;
        let activity_type_bucket = normalize_activity_type(activity.activity_type.as_deref()?)?;
        let duration_seconds = activity
            .elapsed_time_seconds
            .filter(|value| *value > 0)
            .or_else(|| activity.moving_time_seconds.filter(|value| *value > 0))?;
        let duration_bucket_seconds = round_duration_bucket(duration_seconds);
        let distance_bucket_meters = round_distance_bucket(activity.distance_meters);

        Some(Self {
            start_bucket,
            activity_type_bucket,
            duration_bucket_seconds,
            distance_bucket_meters,
            trainer: activity.trainer,
        })
    }

    pub fn as_fingerprint(&self) -> String {
        let distance_bucket = self
            .distance_bucket_meters
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());

        format!(
            "v1:{}|{}|{}|{}|{}",
            self.start_bucket,
            self.activity_type_bucket,
            self.duration_bucket_seconds,
            distance_bucket,
            self.trainer
        )
    }
}

pub fn normalize_external_id(value: Option<&str>) -> Option<String> {
    let normalized = value?.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub fn build_activity_upload_operation_key(
    normalized_external_id: Option<&str>,
    fallback_identity: Option<&str>,
    file_bytes: &[u8],
) -> String {
    if let Some(external_id) = normalized_external_id {
        return format!("external_id:{external_id}");
    }

    if let Some(identity) = fallback_identity {
        return format!("fallback_identity:{identity}");
    }

    let digest = Sha256::digest(file_bytes);
    format!("file_sha256:{digest:x}")
}

fn bucket_start_time(value: &str) -> Option<String> {
    value.get(..16).map(ToString::to_string)
}

fn normalize_activity_type(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn round_duration_bucket(duration_seconds: i32) -> i32 {
    let rounded = ((duration_seconds + 15) / 30) * 30;
    rounded.max(30)
}

pub fn round_distance_bucket(distance_meters: Option<f64>) -> Option<i32> {
    let distance_meters = distance_meters?;
    if !distance_meters.is_finite() || distance_meters <= 0.0 {
        return None;
    }

    let rounded = (distance_meters / 100.0).round() * 100.0;
    if rounded > i32::MAX as f64 {
        return None;
    }

    Some(rounded as i32)
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
            Self::RaceA => "RACE_A",
            Self::RaceB => "RACE_B",
            Self::RaceC => "RACE_C",
            Self::Note => "NOTE",
            Self::Target => "TARGET",
            Self::Season => "SEASON",
            Self::Other => "OTHER",
        }
    }

    pub fn from_api_str(value: &str) -> Self {
        // Reject upstream-only categories (RACE_A/B/C) that are not part of
        // the public REST contract. Map unknown values to Other rather than
        // silently accepting internal-only categories.
        match value {
            "RACE_A" | "RACE_B" | "RACE_C" => Self::Other,
            _ => Self::from_str(value).unwrap_or(Self::Other),
        }
    }

    pub fn from_upstream_str(value: &str) -> Self {
        match value {
            "RACE_A" => Self::RaceA,
            "RACE_B" => Self::RaceB,
            "RACE_C" => Self::RaceC,
            other => Self::from_api_str(other),
        }
    }
}

impl FromStr for EventCategory {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "WORKOUT" => Ok(Self::Workout),
            "RACE" => Ok(Self::Race),
            "RACE_A" => Ok(Self::RaceA),
            "RACE_B" => Ok(Self::RaceB),
            "RACE_C" => Ok(Self::RaceC),
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
    RaceA,
    RaceB,
    RaceC,
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
    pub event_type: Option<String>,
    pub name: Option<String>,
    pub category: EventCategory,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

impl Event {
    pub fn structured_workout_text(&self) -> Option<&str> {
        self.workout_doc
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                self.description
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateEvent {
    pub category: EventCategory,
    pub start_date_local: String,
    pub event_type: Option<String>,
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
    pub event_type: Option<String>,
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
    pub details_unavailable_reason: Option<String>,
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
