use serde::Serialize;

/// Recovery zone threshold: steps at or below 55% FTP are recovery (Z1), per the
/// canonical zone table in `intervals/workout/parser.rs:zone_for_percent`.
pub const RECOVERY_PERCENT_FTP: f64 = 55.0;

/// Power below this is treated as an unplanned drop during a work step
/// (aligner cost masking + anomaly detection).
pub fn work_power_drop_threshold(target_power_min: i32) -> i32 {
    (f64::from(target_power_min) * 0.5) as i32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Work,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    CoastingStop,
    CoastingTurn,
    SignificantPowerDrop,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlannedStep {
    pub name: String,
    pub step_type: StepType,
    pub target_power_min: i32,
    pub target_power_max: i32,
    pub planned_duration_seconds: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CadenceRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkoutAnomaly {
    pub offset_seconds: i32,
    pub duration_seconds: i32,
    pub avg_power: i32,
    pub avg_cadence: i32,
    #[serde(rename = "type")]
    pub anomaly_type: AnomalyType,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlignedInterval {
    pub interval_index: usize,
    pub planned_step: PlannedStep,
    pub actual_duration_seconds: i32,
    pub avg_power: i32,
    pub normalized_power: i32,
    pub avg_cadence: i32,
    pub cadence_range: CadenceRange,
    pub anomalies: Vec<WorkoutAnomaly>,
}
