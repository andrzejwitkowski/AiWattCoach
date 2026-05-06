use serde_json::json;

use crate::domain::completed_workouts::CompletedWorkoutPowerCurve;

pub(super) fn build_power_curve_response(
    date: &str,
    workout_id: &str,
    workout_name: Option<&str>,
    curve: &CompletedWorkoutPowerCurve,
    source: &str,
) -> String {
    let mut resp = json!({
        "date": date,
        "workout_id": workout_id,
        "resolution_seconds": curve.resolution_seconds,
        "sample_period_seconds": curve.sample_period_seconds,
        "source": source,
        "source_samples": curve.source_samples,
        "valid_power_samples": curve.valid_power_samples,
        "duration_start_seconds": curve.duration_start_seconds,
        "duration_step_seconds": curve.duration_step_seconds,
        "max_average_watts": curve.max_average_watts,
    });
    if let Some(name) = workout_name {
        resp["workout_name"] = json!(name);
    }
    resp.to_string()
}
