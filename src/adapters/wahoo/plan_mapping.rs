use serde::Serialize;

use crate::domain::training_plan::TrainingPlanProjectedDay;

const PLAN_JSON_VERSION: &str = "1.0.0";
const WORKOUT_TYPE_FAMILY_BIKING: i32 = 0;
const WORKOUT_TYPE_LOCATION_OUTDOOR: i32 = 1;

#[derive(Serialize)]
struct PlanFile {
    header: PlanHeader,
    intervals: Vec<PlanInterval>,
}

#[derive(Serialize)]
struct PlanHeader {
    name: String,
    version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    workout_type_family: i32,
    workout_type_location: i32,
    ftp: i32,
    duration_s: i32,
}

#[derive(Serialize)]
struct PlanInterval {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    exit_trigger_type: &'static str,
    exit_trigger_value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    intensity_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    targets: Vec<PlanTarget>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    intervals: Vec<PlanInterval>,
}

#[derive(Serialize)]
struct PlanTarget {
    r#type: &'static str,
    low: f64,
    high: f64,
}

pub fn build_plan_file_json(
    projected_day: &TrainingPlanProjectedDay,
    ftp_watts: u32,
) -> Result<String, String> {
    let workout = projected_day
        .workout
        .as_ref()
        .ok_or_else(|| "planned workout is missing workout body".to_string())?;
    let name =
        projected_workout_name(projected_day).unwrap_or_else(|| "Planned workout".to_string());
    let intervals = build_plan_intervals(&workout.lines)?;
    let duration_s = total_duration_seconds(&intervals);
    let plan = PlanFile {
        header: PlanHeader {
            name,
            version: PLAN_JSON_VERSION,
            description: projected_day.rest_day_reason.clone(),
            workout_type_family: WORKOUT_TYPE_FAMILY_BIKING,
            workout_type_location: WORKOUT_TYPE_LOCATION_OUTDOOR,
            ftp: i32::try_from(ftp_watts)
                .map_err(|_| "configured FTP is too large for Wahoo plan header".to_string())?,
            duration_s,
        },
        intervals,
    };
    serde_json::to_string(&plan).map_err(|error| error.to_string())
}

fn build_plan_intervals(
    lines: &[crate::domain::intervals::PlannedWorkoutLine],
) -> Result<Vec<PlanInterval>, String> {
    let mut intervals = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        match &lines[index] {
            crate::domain::intervals::PlannedWorkoutLine::Text(_) => {
                index += 1;
            }
            crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
                intervals.push(step_to_plan_interval(None, step));
                index += 1;
            }
            crate::domain::intervals::PlannedWorkoutLine::Repeat(repeat) => {
                let mut child_steps = Vec::new();
                let mut child_index = index + 1;
                while child_index < lines.len() {
                    match &lines[child_index] {
                        crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
                            child_steps.push(step_to_plan_interval(None, step));
                            child_index += 1;
                        }
                        crate::domain::intervals::PlannedWorkoutLine::Text(_) => break,
                        crate::domain::intervals::PlannedWorkoutLine::Repeat(_) => break,
                    }
                }
                if child_steps.is_empty() {
                    return Err(format!(
                        "repeat block '{}' has no steps to sync",
                        repeat.title.as_deref().unwrap_or("repeat")
                    ));
                }
                intervals.push(PlanInterval {
                    name: repeat.title.clone(),
                    exit_trigger_type: "repeat",
                    exit_trigger_value: repeat.count.saturating_sub(1) as f64,
                    intensity_type: None,
                    targets: Vec::new(),
                    intervals: child_steps,
                });
                index = child_index;
            }
        }
    }
    if intervals.is_empty() {
        return Err("planned workout has no syncable steps".to_string());
    }
    Ok(intervals)
}

fn step_to_plan_interval(
    name: Option<String>,
    step: &crate::domain::intervals::PlannedWorkoutStep,
) -> PlanInterval {
    PlanInterval {
        name,
        exit_trigger_type: "time",
        exit_trigger_value: step.duration_seconds as f64,
        intensity_type: Some(map_intensity_type(step)),
        targets: vec![map_target(&step.target)],
        intervals: Vec::new(),
    }
}

fn map_intensity_type(step: &crate::domain::intervals::PlannedWorkoutStep) -> &'static str {
    match &step.target {
        crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { max, .. } => {
            if *max <= 55.0 {
                "recover"
            } else if *max < 90.0 {
                "active"
            } else if *max <= 105.0 {
                "ftp"
            } else {
                "map"
            }
        }
        crate::domain::intervals::PlannedWorkoutTarget::WattsRange { .. } => "active",
    }
}

fn map_target(target: &crate::domain::intervals::PlannedWorkoutTarget) -> PlanTarget {
    match target {
        crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max } => PlanTarget {
            r#type: "ftp",
            low: min / 100.0,
            high: max / 100.0,
        },
        crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max } => PlanTarget {
            r#type: "watts",
            low: *min as f64,
            high: *max as f64,
        },
    }
}

fn total_duration_seconds(intervals: &[PlanInterval]) -> i32 {
    intervals.iter().map(interval_duration_seconds).sum()
}

fn interval_duration_seconds(interval: &PlanInterval) -> i32 {
    match interval.exit_trigger_type {
        "time" => interval.exit_trigger_value.round() as i32,
        "repeat" => {
            let child_total: i32 = interval
                .intervals
                .iter()
                .map(interval_duration_seconds)
                .sum();
            child_total.saturating_mul(interval.exit_trigger_value.round() as i32 + 1)
        }
        _ => 0,
    }
}

fn projected_workout_name(projected_day: &TrainingPlanProjectedDay) -> Option<String> {
    projected_day.workout.as_ref().and_then(|workout| {
        workout.lines.iter().find_map(|line| match line {
            crate::domain::intervals::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
            _ => None,
        })
    })
}
