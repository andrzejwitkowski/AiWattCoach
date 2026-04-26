use sha2::{Digest, Sha256};

use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics},
    external_sync::{ExternalCompletedWorkoutImport, ExternalImportCommand, ExternalProvider},
    wahoo::{WahooWorkout, WahooWorkoutSummary},
};

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
        workout.workout_token.clone(),
        is_trainer_workout(workout.workout_type_id),
        round_optional_i32(summary.duration_total_seconds)
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
    if let Some(summary) = &workout.workout_summary {
        hasher.update(summary.id.to_string());
        if let Some(updated_at) = &summary.updated_at {
            hasher.update(updated_at.as_bytes());
        }
        if let Some(file) = &summary.file {
            hasher.update(file.url.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::map_workout_to_import_command;
    use crate::domain::wahoo::{WahooFileReference, WahooWorkout, WahooWorkoutSummary};

    fn sample_workout() -> WahooWorkout {
        WahooWorkout {
            id: 56_519,
            starts: "2023-11-14T08:00:00.000Z".to_string(),
            minutes: Some(60),
            name: Some("Wahoo Ride".to_string()),
            plan_id: None,
            plan_ids: Vec::new(),
            route_id: None,
            workout_token: Some("token-1".to_string()),
            workout_type_id: Some(0),
            workout_summary: Some(WahooWorkoutSummary {
                id: 8_297,
                name: Some("Wahoo Ride".to_string()),
                ascent_meters: Some(450.0),
                cadence_avg_rpm: Some(50.0),
                calories: Some(1500.0),
                distance_meters: Some(24_909.71),
                duration_active_seconds: Some(179.0),
                duration_paused_seconds: Some(95.0),
                duration_total_seconds: Some(275.0),
                heart_rate_avg_bpm: Some(100.0),
                normalized_power_watts: Some(150.0),
                training_stress_score: Some(304.9),
                average_power_watts: Some(94.59),
                speed_avg_mps: Some(10.75),
                total_work_joules: Some(1_041_480.0),
                time_zone: Some("America/Denver".to_string()),
                manual: false,
                edited: false,
                fitness_app_id: Some(1002),
                file: Some(WahooFileReference {
                    url: "https://example.test/file.fit".to_string(),
                }),
                created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
                updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            }),
            created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        }
    }

    #[test]
    fn map_workout_to_import_command_uses_wahoo_canonical_identity() {
        let mut workout = sample_workout();
        workout.plan_id = Some(7001);
        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert_eq!(import.workout.completed_workout_id, "wahoo-workout:56519");
        assert_eq!(import.workout.source_activity_id.as_deref(), Some("56519"));
        assert_eq!(import.wahoo_workout_token.as_deref(), Some("token-1"));
        assert_eq!(import.wahoo_plan_id, Some(7001));
        assert_eq!(
            import.workout.details_unavailable_reason.as_deref(),
            Some("Detailed Wahoo workout data is still being processed. Please check back soon.")
        );
    }

    #[test]
    fn map_workout_to_import_command_skips_fit_pending_without_file_url() {
        let mut workout = sample_workout();
        workout.workout_summary.as_mut().unwrap().file = None;

        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert_eq!(import.workout.details_unavailable_reason, None);
    }

    #[test]
    fn map_workout_to_import_command_marks_biking_indoor_as_trainer() {
        let mut workout = sample_workout();
        workout.workout_type_id = Some(super::WahooWorkoutTypeId::BikingIndoor as i64);

        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert!(import.workout.trainer);
    }

    #[test]
    fn map_workout_to_import_command_does_not_classify_motorcycling_as_ride() {
        let mut workout = sample_workout();
        workout.workout_type_id = Some(super::WahooWorkoutTypeId::BikingMotocycling as i64);

        let command = map_workout_to_import_command("user-1", &workout)
            .expect("workout with summary should map");

        let crate::domain::external_sync::ExternalImportCommand::UpsertCompletedWorkout(import) =
            command
        else {
            panic!("expected completed workout import");
        };

        assert_eq!(import.workout.activity_type, None);
    }
}
