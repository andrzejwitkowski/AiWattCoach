use serde::Deserialize;

use crate::domain::wahoo::{WahooFileReference, WahooWorkout, WahooWorkoutSummary};

pub(super) struct WahooWebhookDomainParts {
    pub(super) wahoo_user_id: i64,
    pub(super) workout: WahooWorkout,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WahooWebhookRequest {
    #[serde(rename = "webhook_token")]
    pub(super) webhook_token: String,
    #[serde(rename = "event_type")]
    pub(super) event_type: Option<String>,
    #[serde(rename = "user")]
    pub(super) user: WahooWebhookUser,
    #[serde(rename = "workout_summary")]
    pub(super) workout_summary: Option<WahooWebhookWorkoutSummary>,
    #[serde(rename = "workout")]
    pub(super) workout: WahooWebhookWorkout,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookUser {
    #[serde(rename = "id")]
    pub(super) id: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookWorkout {
    #[serde(rename = "id")]
    pub(super) id: i64,
    #[serde(rename = "starts")]
    pub(super) starts: String,
    #[serde(rename = "minutes")]
    pub(super) minutes: Option<i32>,
    #[serde(rename = "name")]
    pub(super) name: Option<String>,
    #[serde(rename = "plan_id")]
    pub(super) plan_id: Option<i64>,
    #[serde(rename = "plan_ids", default)]
    pub(super) plan_ids: Vec<i64>,
    #[serde(rename = "route_id")]
    pub(super) route_id: Option<i64>,
    #[serde(rename = "workout_token")]
    pub(super) workout_token: Option<String>,
    #[serde(rename = "workout_type_id")]
    pub(super) workout_type_id: Option<i64>,
    #[serde(rename = "created_at")]
    pub(super) created_at: Option<String>,
    #[serde(rename = "updated_at")]
    pub(super) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookWorkoutSummary {
    #[serde(rename = "id")]
    pub(super) id: i64,
    #[serde(rename = "name")]
    pub(super) name: Option<String>,
    #[serde(rename = "ascent_meters")]
    pub(super) ascent_meters: Option<f64>,
    #[serde(rename = "cadence_avg_rpm")]
    pub(super) cadence_avg_rpm: Option<f64>,
    #[serde(rename = "calories")]
    pub(super) calories: Option<f64>,
    #[serde(rename = "distance_meters")]
    pub(super) distance_meters: Option<f64>,
    #[serde(rename = "duration_active_seconds")]
    pub(super) duration_active_seconds: Option<f64>,
    #[serde(rename = "duration_paused_seconds")]
    pub(super) duration_paused_seconds: Option<f64>,
    #[serde(rename = "duration_total_seconds")]
    pub(super) duration_total_seconds: Option<f64>,
    #[serde(rename = "heart_rate_avg_bpm")]
    pub(super) heart_rate_avg_bpm: Option<f64>,
    #[serde(rename = "normalized_power_watts")]
    pub(super) normalized_power_watts: Option<f64>,
    #[serde(rename = "training_stress_score")]
    pub(super) training_stress_score: Option<f64>,
    #[serde(rename = "average_power_watts")]
    pub(super) average_power_watts: Option<f64>,
    #[serde(rename = "speed_avg_mps")]
    pub(super) speed_avg_mps: Option<f64>,
    #[serde(rename = "total_work_joules")]
    pub(super) total_work_joules: Option<f64>,
    #[serde(rename = "time_zone")]
    pub(super) time_zone: Option<String>,
    #[serde(rename = "manual", default)]
    pub(super) manual: bool,
    #[serde(rename = "edited", default)]
    pub(super) edited: bool,
    #[serde(rename = "fitness_app_id")]
    pub(super) fitness_app_id: Option<i64>,
    #[serde(rename = "file")]
    pub(super) file: Option<WahooWebhookFileReference>,
    #[serde(rename = "created_at")]
    pub(super) created_at: Option<String>,
    #[serde(rename = "updated_at")]
    pub(super) updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookFileReference {
    #[serde(rename = "url")]
    pub(super) url: Option<String>,
}

impl WahooWebhookRequest {
    pub(super) fn into_domain_parts(self) -> (Option<String>, WahooWebhookDomainParts) {
        let workout = self.workout.into_domain(self.workout_summary);
        (
            self.event_type,
            WahooWebhookDomainParts {
                wahoo_user_id: self.user.id,
                workout,
            },
        )
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
            file: self
                .file
                .and_then(|file| file.url)
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .map(|url| WahooFileReference { url }),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
