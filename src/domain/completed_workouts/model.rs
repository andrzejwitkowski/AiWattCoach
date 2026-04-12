use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletedWorkoutMetrics {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedWorkoutZoneTime {
    pub zone_id: String,
    pub seconds: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletedWorkoutInterval {
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
pub struct CompletedWorkoutIntervalGroup {
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
pub struct CompletedWorkoutStream {
    pub stream_type: String,
    pub name: Option<String>,
    // Providers may expose one or two raw payload series for the same stream type.
    // The canonical model keeps these as generic primary/secondary series and lets
    // `stream_type` define the business meaning (for example watts, cadence, heartrate).
    pub primary_series: Option<serde_json::Value>,
    pub secondary_series: Option<serde_json::Value>,
    pub value_type_is_array: bool,
    pub custom: bool,
    pub all_null: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletedWorkoutDetails {
    pub intervals: Vec<CompletedWorkoutInterval>,
    pub interval_groups: Vec<CompletedWorkoutIntervalGroup>,
    pub streams: Vec<CompletedWorkoutStream>,
    pub interval_summary: Vec<String>,
    pub skyline_chart: Vec<String>,
    pub power_zone_times: Vec<CompletedWorkoutZoneTime>,
    pub heart_rate_zone_times: Vec<i32>,
    pub pace_zone_times: Vec<i32>,
    pub gap_zone_times: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompletedWorkout {
    pub completed_workout_id: String,
    pub user_id: String,
    pub start_date_local: String,
    pub metrics: CompletedWorkoutMetrics,
    pub details: CompletedWorkoutDetails,
}

impl CompletedWorkout {
    pub fn new(
        completed_workout_id: String,
        user_id: String,
        start_date_local: String,
        metrics: CompletedWorkoutMetrics,
        details: CompletedWorkoutDetails,
    ) -> Self {
        Self {
            completed_workout_id,
            user_id,
            start_date_local,
            metrics,
            details,
        }
    }
}
