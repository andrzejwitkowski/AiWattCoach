use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct EventResponse {
    #[serde(deserialize_with = "deserialize_i64_from_string_or_number")]
    pub id: i64,
    pub start_date_local: String,
    #[serde(default, deserialize_with = "deserialize_lenient_optional_string")]
    pub name: Option<String>,
    pub category: String,
    #[serde(default, deserialize_with = "deserialize_lenient_optional_string")]
    pub description: Option<String>,
    pub indoor: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_lenient_optional_string")]
    pub color: Option<String>,
    #[serde(default, deserialize_with = "deserialize_lenient_optional_string")]
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
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub stream_types: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub interval_summary: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
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
    pub id: StringOrNumber,
    pub secs: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StringOrNumber {
    String(String),
    Number(i64),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum StringListOrSingle {
    List(Vec<String>),
    Single(String),
}

fn deserialize_i64_from_string_or_number<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = StringOrNumber::deserialize(deserializer)?;

    match value {
        StringOrNumber::String(value) => value.parse::<i64>().map_err(serde::de::Error::custom),
        StringOrNumber::Number(value) => Ok(value),
    }
}

fn deserialize_string_from_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

fn deserialize_optional_i32_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;

    value
        .map(|value| match value {
            Value::String(value) => value.parse::<i32>().map_err(serde::de::Error::custom),
            Value::Number(value) => value
                .as_i64()
                .ok_or_else(|| serde::de::Error::custom("expected integer number"))
                .and_then(|value| i32::try_from(value).map_err(serde::de::Error::custom)),
            other => Err(serde::de::Error::custom(format!(
                "expected string or integer number, got {other}"
            ))),
        })
        .transpose()
}

fn deserialize_optional_f64_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;

    value
        .map(|value| match value {
            Value::String(value) => value.parse::<f64>().map_err(serde::de::Error::custom),
            Value::Number(value) => value
                .as_f64()
                .ok_or_else(|| serde::de::Error::custom("expected numeric value")),
            other => Err(serde::de::Error::custom(format!(
                "expected string or number, got {other}"
            ))),
        })
        .transpose()
}

fn deserialize_optional_string_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(|value| match value {
            Value::String(value) => Ok(value),
            Value::Number(value) => Ok(value.to_string()),
            other => Err(serde::de::Error::custom(format!(
                "expected string or number, got {other}"
            ))),
        })
        .transpose()
}

fn deserialize_lenient_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<Value>::deserialize(deserializer)? {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Ok(None),
    }
}

fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<StringListOrSingle>::deserialize(deserializer)?;
    Ok(value.map(|value| match value {
        StringListOrSingle::List(values) => values,
        StringListOrSingle::Single(value) => vec![value],
    }))
}

impl StringOrNumber {
    pub fn into_string(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityIntervalResponse {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub id: Option<i32>,
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub interval_type: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_from_string_or_number"
    )]
    pub group_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub start_index: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub end_index: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub start_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub end_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub moving_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub elapsed_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub distance: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub average_watts: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub weighted_average_watts: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub training_load: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub average_heartrate: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub average_cadence: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub average_speed: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub average_stride: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub zone: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityIntervalGroupResponse {
    #[serde(deserialize_with = "deserialize_string_from_string_or_number")]
    pub id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub count: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub start_index: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub moving_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub elapsed_time: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub distance: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub average_watts: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub weighted_average_watts: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub training_load: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_i32_from_string_or_number"
    )]
    pub average_heartrate: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub average_cadence: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    pub average_speed: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityIntervalsResponse {
    #[serde(default)]
    pub icu_intervals: Vec<ActivityIntervalResponse>,
    #[serde(default, deserialize_with = "deserialize_vec_or_null")]
    pub icu_groups: Vec<ActivityIntervalGroupResponse>,
}

fn deserialize_vec_or_null<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
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
