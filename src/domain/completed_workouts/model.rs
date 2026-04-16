use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompletedWorkoutSeries {
    Integers(Vec<i64>),
    Floats(Vec<f64>),
    Bools(Vec<bool>),
    Strings(Vec<String>),
}

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
    pub primary_series: Option<CompletedWorkoutSeries>,
    pub secondary_series: Option<CompletedWorkoutSeries>,
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
    pub source_activity_id: Option<String>,
    pub planned_workout_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub activity_type: Option<String>,
    pub external_id: Option<String>,
    pub trainer: bool,
    pub duration_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
    pub metrics: CompletedWorkoutMetrics,
    pub details: CompletedWorkoutDetails,
    pub details_unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletedWorkoutError {
    Repository(String),
}

impl std::fmt::Display for CompletedWorkoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CompletedWorkoutError {}

impl CompletedWorkout {
    #[expect(
        clippy::too_many_arguments,
        reason = "canonical completed workouts carry explicit identity, metadata, metrics, and details"
    )]
    pub fn new(
        completed_workout_id: String,
        user_id: String,
        start_date_local: String,
        source_activity_id: Option<String>,
        planned_workout_id: Option<String>,
        name: Option<String>,
        description: Option<String>,
        activity_type: Option<String>,
        external_id: Option<String>,
        trainer: bool,
        duration_seconds: Option<i32>,
        distance_meters: Option<f64>,
        metrics: CompletedWorkoutMetrics,
        details: CompletedWorkoutDetails,
        details_unavailable_reason: Option<String>,
    ) -> Self {
        Self {
            completed_workout_id,
            user_id,
            start_date_local,
            source_activity_id,
            planned_workout_id,
            name,
            description,
            activity_type,
            external_id,
            trainer,
            duration_seconds,
            distance_meters,
            metrics,
            details,
            details_unavailable_reason,
        }
    }
}
