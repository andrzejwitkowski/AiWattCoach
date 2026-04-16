use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ListEventsQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct EventPath {
    pub event_id: i64,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ActivityPath {
    pub activity_id: String,
}

#[derive(Serialize)]
pub(crate) struct EventDto {
    pub id: i64,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "eventDefinition")]
    pub event_definition: EventDefinitionDto,
    #[serde(rename = "actualWorkout")]
    pub actual_workout: Option<ActualWorkoutDto>,
}

#[derive(Serialize)]
pub(crate) struct EventDefinitionDto {
    #[serde(rename = "rawWorkoutDoc")]
    pub raw_workout_doc: Option<String>,
    pub intervals: Vec<IntervalDefinitionDto>,
    pub segments: Vec<WorkoutSegmentDto>,
    pub summary: WorkoutSummaryDto,
}

#[derive(Serialize)]
pub(crate) struct IntervalDefinitionDto {
    pub definition: String,
    #[serde(rename = "repeatCount")]
    pub repeat_count: usize,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: Option<i32>,
    #[serde(rename = "targetPercentFtp")]
    pub target_percent_ftp: Option<f64>,
    #[serde(rename = "zoneId")]
    pub zone_id: Option<i32>,
}

#[derive(Serialize)]
pub(crate) struct WorkoutSegmentDto {
    pub order: usize,
    pub label: String,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: i32,
    #[serde(rename = "startOffsetSeconds")]
    pub start_offset_seconds: i32,
    #[serde(rename = "endOffsetSeconds")]
    pub end_offset_seconds: i32,
    #[serde(rename = "targetPercentFtp")]
    pub target_percent_ftp: Option<f64>,
    #[serde(rename = "zoneId")]
    pub zone_id: Option<i32>,
}

#[derive(Serialize)]
pub(crate) struct WorkoutSummaryDto {
    #[serde(rename = "totalSegments")]
    pub total_segments: usize,
    #[serde(rename = "totalDurationSeconds")]
    pub total_duration_seconds: i32,
    #[serde(rename = "estimatedNormalizedPowerWatts")]
    pub estimated_normalized_power_watts: Option<i32>,
    #[serde(rename = "estimatedAveragePowerWatts")]
    pub estimated_average_power_watts: Option<i32>,
    #[serde(rename = "estimatedIntensityFactor")]
    pub estimated_intensity_factor: Option<f64>,
    #[serde(rename = "estimatedTrainingStressScore")]
    pub estimated_training_stress_score: Option<f64>,
}

#[derive(Serialize)]
pub(crate) struct ActualWorkoutDto {
    #[serde(rename = "activityId")]
    pub activity_id: String,
    #[serde(rename = "activityName")]
    pub activity_name: Option<String>,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    #[serde(rename = "powerValues")]
    pub power_values: Vec<i32>,
    #[serde(rename = "cadenceValues")]
    pub cadence_values: Vec<i32>,
    #[serde(rename = "heartRateValues")]
    pub heart_rate_values: Vec<i32>,
    #[serde(rename = "speedValues")]
    pub speed_values: Vec<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<i32>,
    #[serde(rename = "intensityFactor")]
    pub intensity_factor: Option<f64>,
    #[serde(rename = "complianceScore")]
    pub compliance_score: f64,
    #[serde(rename = "matchedIntervals")]
    pub matched_intervals: Vec<MatchedWorkoutIntervalDto>,
}

#[derive(Serialize)]
pub(crate) struct MatchedWorkoutIntervalDto {
    #[serde(rename = "plannedSegmentOrder")]
    pub planned_segment_order: usize,
    #[serde(rename = "plannedLabel")]
    pub planned_label: String,
    #[serde(rename = "plannedDurationSeconds")]
    pub planned_duration_seconds: i32,
    #[serde(rename = "targetPercentFtp")]
    pub target_percent_ftp: Option<f64>,
    #[serde(rename = "zoneId")]
    pub zone_id: Option<i32>,
    #[serde(rename = "actualIntervalId")]
    pub actual_interval_id: Option<i32>,
    #[serde(rename = "actualStartTimeSeconds")]
    pub actual_start_time_seconds: Option<i32>,
    #[serde(rename = "actualEndTimeSeconds")]
    pub actual_end_time_seconds: Option<i32>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "complianceScore")]
    pub compliance_score: f64,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct CreateEventDto {
    pub category: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
    #[serde(rename = "fileUpload")]
    pub file_upload: Option<EventFileUploadDto>,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct UpdateEventDto {
    pub category: Option<String>,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: Option<String>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub indoor: Option<bool>,
    pub color: Option<String>,
    #[serde(rename = "workoutDoc")]
    pub workout_doc: Option<String>,
    #[serde(rename = "fileUpload")]
    pub file_upload: Option<EventFileUploadDto>,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct EventFileUploadDto {
    pub filename: String,
    #[serde(rename = "fileContents")]
    pub file_contents: Option<String>,
    #[serde(rename = "fileContentsBase64")]
    pub file_contents_base64: Option<String>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityDto {
    pub id: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "activityType")]
    pub activity_type: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "totalElevationGainMeters")]
    pub total_elevation_gain_meters: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    pub trainer: bool,
    pub commute: bool,
    pub race: bool,
    #[serde(rename = "hasHeartRate")]
    pub has_heart_rate: bool,
    #[serde(rename = "streamTypes")]
    pub stream_types: Vec<String>,
    pub tags: Vec<String>,
    pub metrics: ActivityMetricsDto,
    pub details: ActivityDetailsDto,
    #[serde(rename = "detailsUnavailableReason")]
    pub details_unavailable_reason: Option<String>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityMetricsDto {
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "intensityFactor")]
    pub intensity_factor: Option<f64>,
    #[serde(rename = "efficiencyFactor")]
    pub efficiency_factor: Option<f64>,
    #[serde(rename = "variabilityIndex")]
    pub variability_index: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "ftpWatts")]
    pub ftp_watts: Option<i32>,
    #[serde(rename = "totalWorkJoules")]
    pub total_work_joules: Option<i32>,
    pub calories: Option<i32>,
    pub trimp: Option<f64>,
    #[serde(rename = "powerLoad")]
    pub power_load: Option<i32>,
    #[serde(rename = "heartRateLoad")]
    pub heart_rate_load: Option<i32>,
    #[serde(rename = "paceLoad")]
    pub pace_load: Option<i32>,
    #[serde(rename = "strainScore")]
    pub strain_score: Option<f64>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityDetailsDto {
    pub intervals: Vec<ActivityIntervalDto>,
    #[serde(rename = "intervalGroups")]
    pub interval_groups: Vec<ActivityIntervalGroupDto>,
    pub streams: Vec<ActivityStreamDto>,
    #[serde(rename = "intervalSummary")]
    pub interval_summary: Vec<String>,
    #[serde(rename = "skylineChart")]
    pub skyline_chart: Vec<String>,
    #[serde(rename = "powerZoneTimes")]
    pub power_zone_times: Vec<ActivityZoneTimeDto>,
    #[serde(rename = "heartRateZoneTimes")]
    pub heart_rate_zone_times: Vec<i32>,
    #[serde(rename = "paceZoneTimes")]
    pub pace_zone_times: Vec<i32>,
    #[serde(rename = "gapZoneTimes")]
    pub gap_zone_times: Vec<i32>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityIntervalDto {
    pub id: Option<i32>,
    pub label: Option<String>,
    #[serde(rename = "intervalType")]
    pub interval_type: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<i32>,
    #[serde(rename = "endIndex")]
    pub end_index: Option<i32>,
    #[serde(rename = "startTimeSeconds")]
    pub start_time_seconds: Option<i32>,
    #[serde(rename = "endTimeSeconds")]
    pub end_time_seconds: Option<i32>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageStrideMeters")]
    pub average_stride_meters: Option<f64>,
    pub zone: Option<i32>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityIntervalGroupDto {
    pub id: String,
    pub count: Option<i32>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<i32>,
    #[serde(rename = "movingTimeSeconds")]
    pub moving_time_seconds: Option<i32>,
    #[serde(rename = "elapsedTimeSeconds")]
    pub elapsed_time_seconds: Option<i32>,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: Option<f64>,
    #[serde(rename = "averagePowerWatts")]
    pub average_power_watts: Option<i32>,
    #[serde(rename = "normalizedPowerWatts")]
    pub normalized_power_watts: Option<i32>,
    #[serde(rename = "trainingStressScore")]
    pub training_stress_score: Option<f64>,
    #[serde(rename = "averageHeartRateBpm")]
    pub average_heart_rate_bpm: Option<i32>,
    #[serde(rename = "averageCadenceRpm")]
    pub average_cadence_rpm: Option<f64>,
    #[serde(rename = "averageSpeedMps")]
    pub average_speed_mps: Option<f64>,
    #[serde(rename = "averageStrideMeters")]
    pub average_stride_meters: Option<f64>,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityStreamDto {
    #[serde(rename = "streamType")]
    pub stream_type: String,
    pub name: Option<String>,
    pub data: Option<Value>,
    pub data2: Option<Value>,
    #[serde(rename = "valueTypeIsArray")]
    pub value_type_is_array: bool,
    pub custom: bool,
    #[serde(rename = "allNull")]
    pub all_null: bool,
}

#[derive(Serialize)]
pub(in crate::adapters::rest) struct ActivityZoneTimeDto {
    #[serde(rename = "zoneId")]
    pub zone_id: String,
    pub seconds: i32,
}

#[derive(Serialize)]
pub(super) struct UploadActivityResponseDto {
    pub created: bool,
    #[serde(rename = "activityIds")]
    pub activity_ids: Vec<String>,
    pub activities: Vec<ActivityDto>,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct CreateActivityDto {
    pub filename: String,
    #[serde(rename = "fileContentsBase64")]
    pub file_contents_base64: String,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "externalId")]
    pub external_id: Option<String>,
    #[serde(rename = "pairedEventId")]
    pub paired_event_id: Option<i32>,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct UpdateActivityDto {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "activityType")]
    pub activity_type: Option<String>,
    pub trainer: Option<bool>,
    pub commute: Option<bool>,
    pub race: Option<bool>,
}
