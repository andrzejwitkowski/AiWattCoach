use sha2::{Digest, Sha256};

use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics},
    external_sync::{ExternalCompletedWorkoutImport, ExternalImportCommand, ExternalProvider},
};

use super::{WahooWorkout, WahooWorkoutSummary};

const DETAILS_UNAVAILABLE_REASON_WAHOO_FIT_PENDING: &str =
    "Detailed Wahoo workout data is still being processed. Please check back soon.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
enum WahooWorkoutTypeId {
    Biking = 0,
    Running = 1,
    Fe = 2,
    RunningTrack = 3,
    RunningTrail = 4,
    RunningTreadmill = 5,
    Walking = 6,
    WalkingSpeed = 7,
    WalkingNordic = 8,
    Hiking = 9,
    Mountaineering = 10,
    BikingCyclecross = 11,
    BikingIndoor = 12,
    BikingMountain = 13,
    BikingMotocycling = 17,
}

impl WahooWorkoutTypeId {
    fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Biking),
            1 => Some(Self::Running),
            2 => Some(Self::Fe),
            3 => Some(Self::RunningTrack),
            4 => Some(Self::RunningTrail),
            5 => Some(Self::RunningTreadmill),
            6 => Some(Self::Walking),
            7 => Some(Self::WalkingSpeed),
            8 => Some(Self::WalkingNordic),
            9 => Some(Self::Hiking),
            10 => Some(Self::Mountaineering),
            11 => Some(Self::BikingCyclecross),
            12 => Some(Self::BikingIndoor),
            13 => Some(Self::BikingMountain),
            17 => Some(Self::BikingMotocycling),
            _ => None,
        }
    }
}

pub fn map_workout_to_import_command(
    user_id: &str,
    workout: &WahooWorkout,
) -> Option<ExternalImportCommand> {
    let summary = workout.workout_summary.as_ref()?;
    Some(ExternalImportCommand::UpsertCompletedWorkout(Box::new(
        ExternalCompletedWorkoutImport {
            provider: ExternalProvider::Wahoo,
            external_id: workout.id.to_string(),
            normalized_payload_hash: hash_workout(workout),
            intervals_paired_event_id: None,
            marker_sources: [
                workout.workout_token.clone(),
                workout.name.clone(),
                summary.name.clone(),
            ]
            .into_iter()
            .flatten()
            .collect(),
            wahoo_plan_id: workout.plan_id,
            wahoo_workout_token: workout.workout_token.clone(),
            workout: map_workout_to_completed_workout(user_id, workout, summary),
        },
    )))
}

fn map_workout_to_completed_workout(
    user_id: &str,
    workout: &WahooWorkout,
    summary: &WahooWorkoutSummary,
) -> CompletedWorkout {
    CompletedWorkout::new(
        format!("wahoo-workout:{}", workout.id),
        user_id.to_string(),
        workout.starts.clone(),
        Some(workout.id.to_string()),
        None,
        summary.name.clone().or_else(|| workout.name.clone()),
        None,
        map_activity_type(workout.workout_type_id),
        Some(workout.id.to_string()),
        is_trainer_workout(workout.workout_type_id),
        round_optional_i32(summary.duration_total_seconds)
            .or_else(|| round_optional_i32(summary.duration_active_seconds))
            .or_else(|| workout.minutes.map(|minutes| minutes.saturating_mul(60))),
        summary.distance_meters,
        CompletedWorkoutMetrics {
            training_stress_score: round_optional_i32(summary.training_stress_score),
            normalized_power_watts: round_optional_i32(summary.normalized_power_watts),
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: round_optional_i32(summary.average_power_watts),
            ftp_watts: None,
            total_work_joules: round_optional_i32(summary.total_work_joules),
            calories: round_optional_i32(summary.calories),
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        summary
            .file
            .as_ref()
            .filter(|file| !file.url.trim().is_empty())
            .map(|_| DETAILS_UNAVAILABLE_REASON_WAHOO_FIT_PENDING.to_string()),
    )
}

fn round_optional_i32(value: Option<f64>) -> Option<i32> {
    let value = value?;
    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
        None
    } else {
        Some(value.round() as i32)
    }
}

fn map_activity_type(workout_type_id: Option<i64>) -> Option<String> {
    match workout_type_id.and_then(WahooWorkoutTypeId::from_i64) {
        Some(
            WahooWorkoutTypeId::Biking
            | WahooWorkoutTypeId::Fe
            | WahooWorkoutTypeId::BikingCyclecross
            | WahooWorkoutTypeId::BikingIndoor
            | WahooWorkoutTypeId::BikingMountain,
        ) => Some("Ride".to_string()),
        Some(
            WahooWorkoutTypeId::Running
            | WahooWorkoutTypeId::RunningTrack
            | WahooWorkoutTypeId::RunningTrail
            | WahooWorkoutTypeId::RunningTreadmill,
        ) => Some("Run".to_string()),
        Some(
            WahooWorkoutTypeId::Walking
            | WahooWorkoutTypeId::WalkingSpeed
            | WahooWorkoutTypeId::WalkingNordic
            | WahooWorkoutTypeId::Hiking
            | WahooWorkoutTypeId::Mountaineering,
        ) => Some("Walk".to_string()),
        _ => None,
    }
}

fn is_trainer_workout(workout_type_id: Option<i64>) -> bool {
    matches!(
        workout_type_id.and_then(WahooWorkoutTypeId::from_i64),
        Some(
            WahooWorkoutTypeId::Fe
                | WahooWorkoutTypeId::RunningTreadmill
                | WahooWorkoutTypeId::BikingIndoor
        )
    )
}

fn hash_workout(workout: &WahooWorkout) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workout.id.to_string());
    hasher.update(workout.starts.as_bytes());
    hash_optional_i32(&mut hasher, workout.minutes);
    hash_optional_string(&mut hasher, workout.name.as_deref());
    hash_optional_i64(&mut hasher, workout.plan_id);
    for plan_id in &workout.plan_ids {
        hasher.update(plan_id.to_string());
    }
    hash_optional_i64(&mut hasher, workout.route_id);
    hash_optional_string(&mut hasher, workout.workout_token.as_deref());
    hash_optional_i64(&mut hasher, workout.workout_type_id);
    hash_optional_string(&mut hasher, workout.created_at.as_deref());
    hash_optional_string(&mut hasher, workout.updated_at.as_deref());
    if let Some(summary) = &workout.workout_summary {
        hasher.update(summary.id.to_string());
        hash_optional_string(&mut hasher, summary.name.as_deref());
        hash_optional_f64(&mut hasher, summary.ascent_meters);
        hash_optional_f64(&mut hasher, summary.cadence_avg_rpm);
        hash_optional_f64(&mut hasher, summary.calories);
        hash_optional_f64(&mut hasher, summary.distance_meters);
        hash_optional_f64(&mut hasher, summary.duration_active_seconds);
        hash_optional_f64(&mut hasher, summary.duration_paused_seconds);
        hash_optional_f64(&mut hasher, summary.duration_total_seconds);
        hash_optional_f64(&mut hasher, summary.heart_rate_avg_bpm);
        hash_optional_f64(&mut hasher, summary.normalized_power_watts);
        hash_optional_f64(&mut hasher, summary.training_stress_score);
        hash_optional_f64(&mut hasher, summary.average_power_watts);
        hash_optional_f64(&mut hasher, summary.speed_avg_mps);
        hash_optional_f64(&mut hasher, summary.total_work_joules);
        hash_optional_string(&mut hasher, summary.time_zone.as_deref());
        hasher.update(if summary.manual { b"1" } else { b"0" });
        hasher.update(if summary.edited { b"1" } else { b"0" });
        hash_optional_i64(&mut hasher, summary.fitness_app_id);
        if let Some(file) = &summary.file {
            hasher.update(file.url.as_bytes());
        }
        hash_optional_string(&mut hasher, summary.created_at.as_deref());
        hash_optional_string(&mut hasher, summary.updated_at.as_deref());
    }
    format!("{:x}", hasher.finalize())
}

fn hash_optional_string(hasher: &mut Sha256, value: Option<&str>) {
    hasher.update(value.unwrap_or_default().as_bytes());
}

fn hash_optional_i64(hasher: &mut Sha256, value: Option<i64>) {
    hasher.update(
        value
            .map(|value| value.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
}

fn hash_optional_i32(hasher: &mut Sha256, value: Option<i32>) {
    hasher.update(
        value
            .map(|value| value.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
}

fn hash_optional_f64(hasher: &mut Sha256, value: Option<f64>) {
    hasher.update(
        value
            .map(|value| format!("{value:.6}"))
            .unwrap_or_default()
            .as_bytes(),
    );
}
