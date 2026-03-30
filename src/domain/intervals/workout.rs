mod matching;
mod parser;

use serde::{Deserialize, Serialize};

pub use matching::find_best_activity_match;
pub use parser::parse_workout_doc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedWorkoutDoc {
    pub intervals: Vec<WorkoutIntervalDefinition>,
    pub segments: Vec<WorkoutSegment>,
    pub summary: WorkoutSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutIntervalDefinition {
    pub definition: String,
    pub repeat_count: usize,
    pub duration_seconds: Option<i32>,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSegment {
    pub order: usize,
    pub label: String,
    pub duration_seconds: i32,
    pub start_offset_seconds: i32,
    pub end_offset_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSummary {
    pub total_segments: usize,
    pub total_duration_seconds: i32,
    pub estimated_normalized_power_watts: Option<i32>,
    pub estimated_average_power_watts: Option<i32>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_training_stress_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActualWorkoutMatch {
    pub activity_id: String,
    pub activity_name: Option<String>,
    pub start_date_local: String,
    pub power_values: Vec<i32>,
    pub cadence_values: Vec<i32>,
    pub heart_rate_values: Vec<i32>,
    pub speed_values: Vec<f64>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub training_stress_score: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub compliance_score: f64,
    pub matched_intervals: Vec<MatchedWorkoutInterval>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchedWorkoutInterval {
    pub planned_segment_order: usize,
    pub planned_label: String,
    pub planned_duration_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
    pub actual_interval_id: Option<i32>,
    pub actual_start_time_seconds: Option<i32>,
    pub actual_end_time_seconds: Option<i32>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub average_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub compliance_score: f64,
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}
