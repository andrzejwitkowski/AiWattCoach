use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct EventResponse {
    pub id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    pub workout_doc: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateEventRequest {
    pub category: String,
    pub start_date_local: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub indoor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_contents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_contents_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateEventRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_local: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indoor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_contents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_contents_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityResponse {
    pub id: String,
    pub start_date_local: String,
    pub start_date: Option<String>,
    #[serde(rename = "type")]
    pub activity_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub source: Option<String>,
    pub external_id: Option<String>,
    pub device_name: Option<String>,
    pub icu_athlete_id: Option<String>,
    pub distance: Option<f64>,
    pub moving_time: Option<i32>,
    pub elapsed_time: Option<i32>,
    pub total_elevation_gain: Option<f64>,
    pub total_elevation_loss: Option<f64>,
    pub average_speed: Option<f64>,
    pub max_speed: Option<f64>,
    pub average_heartrate: Option<i32>,
    pub max_heartrate: Option<i32>,
    pub average_cadence: Option<f64>,
    pub trainer: Option<bool>,
    pub commute: Option<bool>,
    pub race: Option<bool>,
    pub has_heartrate: Option<bool>,
    pub stream_types: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub interval_summary: Option<Vec<String>>,
    pub skyline_chart_bytes: Option<Vec<String>>,
    pub icu_zone_times: Option<Vec<ZoneTimeResponse>>,
    pub icu_hr_zone_times: Option<Vec<i32>>,
    pub pace_zone_times: Option<Vec<i32>>,
    pub gap_zone_times: Option<Vec<i32>>,
    pub icu_training_load: Option<i32>,
    pub icu_weighted_avg_watts: Option<i32>,
    pub icu_intensity: Option<f64>,
    pub icu_efficiency_factor: Option<f64>,
    pub icu_variability_index: Option<f64>,
    pub icu_average_watts: Option<i32>,
    pub icu_ftp: Option<i32>,
    pub icu_joules: Option<i32>,
    pub calories: Option<i32>,
    pub trimp: Option<f64>,
    pub power_load: Option<i32>,
    pub hr_load: Option<i32>,
    pub pace_load: Option<i32>,
    pub strain_score: Option<f64>,
    pub icu_intervals: Option<Vec<ActivityIntervalResponse>>,
    pub icu_groups: Option<Vec<ActivityIntervalGroupResponse>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ZoneTimeResponse {
    pub id: String,
    pub secs: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityIntervalResponse {
    pub id: Option<i32>,
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub interval_type: Option<String>,
    pub group_id: Option<String>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
    pub moving_time: Option<i32>,
    pub elapsed_time: Option<i32>,
    pub distance: Option<f64>,
    pub average_watts: Option<i32>,
    pub weighted_average_watts: Option<i32>,
    pub training_load: Option<f64>,
    pub average_heartrate: Option<i32>,
    pub average_cadence: Option<f64>,
    pub average_speed: Option<f64>,
    pub average_stride: Option<f64>,
    pub zone: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityIntervalGroupResponse {
    pub id: String,
    pub count: Option<i32>,
    pub start_index: Option<i32>,
    pub moving_time: Option<i32>,
    pub elapsed_time: Option<i32>,
    pub distance: Option<f64>,
    pub average_watts: Option<i32>,
    pub weighted_average_watts: Option<i32>,
    pub training_load: Option<f64>,
    pub average_heartrate: Option<i32>,
    pub average_cadence: Option<f64>,
    pub average_speed: Option<f64>,
    pub average_stride: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityStreamResponse {
    #[serde(rename = "type")]
    pub stream_type: String,
    pub name: Option<String>,
    pub data: Option<Value>,
    pub data2: Option<Value>,
    #[serde(default, rename = "valueTypeIsArray")]
    pub value_type_is_array: bool,
    #[serde(default)]
    pub custom: bool,
    #[serde(default, rename = "allNull")]
    pub all_null: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UploadResponse {
    pub activities: Option<Vec<ActivityIdResponse>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ActivityIdResponse {
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateActivityRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub activity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trainer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub race: Option<bool>,
}
