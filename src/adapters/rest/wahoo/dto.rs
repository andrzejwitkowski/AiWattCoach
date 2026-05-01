use serde::Deserialize;

use crate::domain::wahoo::{WahooFileReference, WahooWorkout, WahooWorkoutSummary};

#[derive(Debug, Deserialize)]
pub(crate) struct WahooWebhookRequest {
    pub(super) webhook_token: String,
    pub(super) event_type: Option<String>,
    pub(super) user: WahooWebhookUser,
    pub(super) workout_summary: Option<WahooWebhookWorkoutSummary>,
    pub(super) workout: WahooWebhookWorkout,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookUser {
    pub(super) id: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookWorkout {
    pub(super) id: i64,
    pub(super) starts: String,
    pub(super) minutes: Option<i32>,
    pub(super) name: Option<String>,
    pub(super) plan_id: Option<i64>,
    #[serde(default)]
    pub(super) plan_ids: Vec<i64>,
    pub(super) route_id: Option<i64>,
    pub(super) workout_token: Option<String>,
    pub(super) workout_type_id: Option<i64>,
    pub(super) created_at: Option<String>,
    pub(super) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookWorkoutSummary {
    pub(super) id: i64,
    pub(super) name: Option<String>,
    pub(super) ascent_meters: Option<f64>,
    pub(super) cadence_avg_rpm: Option<f64>,
    pub(super) calories: Option<f64>,
    pub(super) distance_meters: Option<f64>,
    pub(super) duration_active_seconds: Option<f64>,
    pub(super) duration_paused_seconds: Option<f64>,
    pub(super) duration_total_seconds: Option<f64>,
    pub(super) heart_rate_avg_bpm: Option<f64>,
    pub(super) normalized_power_watts: Option<f64>,
    pub(super) training_stress_score: Option<f64>,
    pub(super) average_power_watts: Option<f64>,
    pub(super) speed_avg_mps: Option<f64>,
    pub(super) total_work_joules: Option<f64>,
    pub(super) time_zone: Option<String>,
    #[serde(default)]
    pub(super) manual: bool,
    #[serde(default)]
    pub(super) edited: bool,
    pub(super) fitness_app_id: Option<i64>,
    pub(super) file: Option<WahooWebhookFileReference>,
    pub(super) created_at: Option<String>,
    pub(super) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookFileReference {
    pub(super) url: String,
}

impl WahooWebhookRequest {
    pub(super) fn into_domain_parts(self) -> (Option<String>, i64, WahooWorkout) {
        let workout = self.workout.into_domain(self.workout_summary);
        (self.event_type, self.user.id, workout)
    }
}

impl WahooWebhookWorkout {
    fn into_domain(self, workout_summary: Option<WahooWebhookWorkoutSummary>) -> WahooWorkout {
        WahooWorkout {
            id: self.id,
            starts: self.starts,
            minutes: self.minutes,
            name: self.name,
            plan_id: self.plan_id,
            plan_ids: self.plan_ids,
            route_id: self.route_id,
            workout_token: self.workout_token,
            workout_type_id: self.workout_type_id,
            workout_summary: workout_summary.map(WahooWebhookWorkoutSummary::into_domain),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl WahooWebhookWorkoutSummary {
    fn into_domain(self) -> WahooWorkoutSummary {
        WahooWorkoutSummary {
            id: self.id,
            name: self.name,
            ascent_meters: self.ascent_meters,
            cadence_avg_rpm: self.cadence_avg_rpm,
            calories: self.calories,
            distance_meters: self.distance_meters,
            duration_active_seconds: self.duration_active_seconds,
            duration_paused_seconds: self.duration_paused_seconds,
            duration_total_seconds: self.duration_total_seconds,
            heart_rate_avg_bpm: self.heart_rate_avg_bpm,
            normalized_power_watts: self.normalized_power_watts,
            training_stress_score: self.training_stress_score,
            average_power_watts: self.average_power_watts,
            speed_avg_mps: self.speed_avg_mps,
            total_work_joules: self.total_work_joules,
            time_zone: self.time_zone,
            manual: self.manual,
            edited: self.edited,
            fitness_app_id: self.fitness_app_id,
            file: self.file.map(|file| WahooFileReference { url: file.url }),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
