use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::Value;

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
    pub(super) workout: Option<WahooWebhookWorkout>,
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
    #[serde(
        rename = "plan_ids",
        default,
        deserialize_with = "deserialize_vec_or_default"
    )]
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
    #[serde(rename = "started_at")]
    pub(super) started_at: Option<String>,
    #[serde(rename = "name")]
    pub(super) name: Option<String>,
    #[serde(
        rename = "ascent_meters",
        alias = "ascent_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) ascent_meters: Option<f64>,
    #[serde(
        rename = "cadence_avg_rpm",
        alias = "cadence_avg",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) cadence_avg_rpm: Option<f64>,
    #[serde(
        rename = "calories",
        alias = "calories_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) calories: Option<f64>,
    #[serde(
        rename = "distance_meters",
        alias = "distance_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) distance_meters: Option<f64>,
    #[serde(
        rename = "duration_active_seconds",
        alias = "duration_active_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) duration_active_seconds: Option<f64>,
    #[serde(
        rename = "duration_paused_seconds",
        alias = "duration_paused_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) duration_paused_seconds: Option<f64>,
    #[serde(
        rename = "duration_total_seconds",
        alias = "duration_total_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) duration_total_seconds: Option<f64>,
    #[serde(
        rename = "heart_rate_avg_bpm",
        alias = "heart_rate_avg",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) heart_rate_avg_bpm: Option<f64>,
    #[serde(
        rename = "normalized_power_watts",
        alias = "power_bike_np_last",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) normalized_power_watts: Option<f64>,
    #[serde(
        rename = "training_stress_score",
        alias = "power_bike_tss_last",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) training_stress_score: Option<f64>,
    #[serde(
        rename = "average_power_watts",
        alias = "power_avg",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) average_power_watts: Option<f64>,
    #[serde(
        rename = "speed_avg_mps",
        alias = "speed_avg",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) speed_avg_mps: Option<f64>,
    #[serde(
        rename = "total_work_joules",
        alias = "work_accum",
        default,
        deserialize_with = "deserialize_optional_f64"
    )]
    pub(super) total_work_joules: Option<f64>,
    #[serde(rename = "time_zone")]
    pub(super) time_zone: Option<String>,
    #[serde(
        rename = "manual",
        default,
        deserialize_with = "deserialize_bool_or_default"
    )]
    pub(super) manual: bool,
    #[serde(
        rename = "edited",
        default,
        deserialize_with = "deserialize_bool_or_default"
    )]
    pub(super) edited: bool,
    #[serde(rename = "fitness_app_id")]
    pub(super) fitness_app_id: Option<i64>,
    #[serde(rename = "file")]
    pub(super) file: Option<WahooWebhookFileReference>,
    #[serde(rename = "created_at")]
    pub(super) created_at: Option<String>,
    #[serde(rename = "updated_at")]
    pub(super) updated_at: Option<String>,
    #[serde(rename = "workout")]
    pub(super) workout: Option<WahooWebhookWorkout>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WahooWebhookFileReference {
    #[serde(rename = "url")]
    pub(super) url: Option<String>,
}

impl WahooWebhookRequest {
    pub(super) fn into_domain_parts(
        self,
    ) -> Result<(Option<String>, WahooWebhookDomainParts), &'static str> {
        let workout = match (self.workout, self.workout_summary) {
            (Some(workout), Some(workout_summary)) => {
                workout.into_domain(Some(workout_summary.into_domain()))
            }
            (Some(workout), None) => workout.into_domain(None),
            (None, Some(workout_summary)) => workout_summary.into_summary_only_workout()?,
            (None, None) => return Err("missing workout payload"),
        };

        Ok((
            self.event_type,
            WahooWebhookDomainParts {
                wahoo_user_id: self.user.id,
                workout,
            },
        ))
    }
}

impl WahooWebhookWorkout {
    fn into_domain(self, workout_summary: Option<WahooWorkoutSummary>) -> WahooWorkout {
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
            workout_summary,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

fn deserialize_bool_or_default<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

fn deserialize_vec_or_default<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_f64()
            .map(Some)
            .ok_or_else(|| D::Error::custom("invalid numeric value")),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok(None);
            }

            value.parse::<f64>().map(Some).map_err(D::Error::custom)
        }
        Some(other) => Err(D::Error::custom(format!(
            "expected number or string, got {other}"
        ))),
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

    fn into_summary_only_workout(self) -> Result<WahooWorkout, &'static str> {
        let WahooWebhookWorkoutSummary {
            id,
            started_at,
            name,
            ascent_meters,
            cadence_avg_rpm,
            calories,
            distance_meters,
            duration_active_seconds,
            duration_paused_seconds,
            duration_total_seconds,
            heart_rate_avg_bpm,
            normalized_power_watts,
            training_stress_score,
            average_power_watts,
            speed_avg_mps,
            total_work_joules,
            time_zone,
            manual,
            edited,
            fitness_app_id,
            file,
            created_at,
            updated_at,
            workout,
        } = self;

        let workout = workout.ok_or("missing workout payload")?;
        let file = file
            .and_then(|file| file.url)
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .map(|url| WahooFileReference { url });
        let summary = WahooWorkoutSummary {
            id,
            name,
            ascent_meters,
            cadence_avg_rpm,
            calories,
            distance_meters,
            duration_active_seconds,
            duration_paused_seconds,
            duration_total_seconds,
            heart_rate_avg_bpm,
            normalized_power_watts,
            training_stress_score,
            average_power_watts,
            speed_avg_mps,
            total_work_joules,
            time_zone,
            manual,
            edited,
            fitness_app_id,
            file,
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
        };

        Ok(WahooWorkout {
            id,
            starts: started_at.unwrap_or(workout.starts),
            minutes: workout.minutes,
            name: workout.name,
            plan_id: workout.plan_id,
            plan_ids: workout.plan_ids,
            route_id: workout.route_id,
            workout_token: workout.workout_token,
            workout_type_id: workout.workout_type_id,
            workout_summary: Some(summary),
            created_at,
            updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WahooWebhookRequest;

    #[test]
    fn real_summary_only_payload_maps_to_completed_workout_fields() {
        let payload = r#"{
            "event_type": "workout_summary",
            "webhook_token": "secret-token",
            "user": { "id": 616126 },
            "workout_summary": {
                "id": 402756448,
                "started_at": "2026-05-02T08:14:29.000Z",
                "ascent_accum": "36.0",
                "cadence_avg": "76.0",
                "calories_accum": "464.0",
                "distance_accum": "20959.15",
                "duration_active_accum": "2405.0",
                "duration_paused_accum": "36.0",
                "duration_total_accum": "2441.0",
                "heart_rate_avg": null,
                "power_bike_np_last": "221.0",
                "power_bike_tss_last": "28.1",
                "power_avg": "193.0",
                "speed_avg": "8.72",
                "work_accum": "464094.0",
                "fitness_app_id": 14,
                "time_zone": "Europe/Warsaw",
                "created_at": "2026-05-02T10:11:08.000Z",
                "updated_at": "2026-05-02T10:11:13.000Z",
                "file": {
                    "url": "https://cdn.wahooligan.com/wahoo-cloud/production/uploads/workout_file/file/test.fit"
                },
                "workout": {
                    "id": 451769692,
                    "starts": "2026-05-02T21:00:00.000Z",
                    "minutes": 40,
                    "name": "Race Openers",
                    "created_at": "2026-05-01T11:50:19.000Z",
                    "updated_at": "2026-05-01T11:50:19.000Z",
                    "plan_id": 13449478,
                    "workout_token": "icu_107574759",
                    "workout_type_id": 0,
                    "fitness_app_id": 1199
                }
            }
        }"#;

        let request: WahooWebhookRequest = serde_json::from_str(payload).unwrap();
        let (event_type, parts) = request.into_domain_parts().unwrap();

        assert_eq!(event_type.as_deref(), Some("workout_summary"));
        assert_eq!(parts.wahoo_user_id, 616_126);
        assert_eq!(parts.workout.id, 402_756_448);
        assert_eq!(parts.workout.starts, "2026-05-02T08:14:29.000Z");
        assert_eq!(parts.workout.minutes, Some(40));
        assert_eq!(parts.workout.name.as_deref(), Some("Race Openers"));
        assert_eq!(parts.workout.plan_id, Some(13_449_478));
        assert_eq!(
            parts.workout.workout_token.as_deref(),
            Some("icu_107574759")
        );
        assert_eq!(parts.workout.workout_type_id, Some(0));

        let summary = parts.workout.workout_summary.expect("summary should exist");
        assert_eq!(summary.id, 402_756_448);
        assert_eq!(summary.distance_meters, Some(20_959.15));
        assert_eq!(summary.duration_total_seconds, Some(2_441.0));
        assert_eq!(summary.normalized_power_watts, Some(221.0));
        assert_eq!(summary.training_stress_score, Some(28.1));
        assert_eq!(
            summary.file.expect("file should exist").url,
            "https://cdn.wahooligan.com/wahoo-cloud/production/uploads/workout_file/file/test.fit"
        );
    }
}
