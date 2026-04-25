use crate::domain::wahoo::{
    BoxFuture, WahooApiPort, WahooError, WahooFileReference, WahooOAuthPort, WahooToken,
    WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
};

const DEV_AUTH_CODE: &str = "dev-wahoo-auth";

#[derive(Clone)]
pub struct DevWahooOAuthClient;

impl WahooOAuthPort for DevWahooOAuthClient {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        Ok(format!(
            "/api/wahoo/callback?state={state}&code={DEV_AUTH_CODE}"
        ))
    }

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let code = code.to_string();
        Box::pin(async move {
            if code != DEV_AUTH_CODE {
                return Err(WahooError::External("invalid dev Wahoo code".to_string()));
            }

            Ok(WahooToken {
                access_token: "dev-wahoo-access-token".to_string(),
                refresh_token: "dev-wahoo-refresh-token".to_string(),
                expires_at_epoch_seconds: chrono::Utc::now().timestamp() + 7200,
            })
        })
    }

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let refresh_token = refresh_token.to_string();
        Box::pin(async move {
            if refresh_token.trim().is_empty() {
                return Err(WahooError::NotConnected);
            }

            Ok(WahooToken {
                access_token: "dev-wahoo-access-token-refreshed".to_string(),
                refresh_token: "dev-wahoo-refresh-token-refreshed".to_string(),
                expires_at_epoch_seconds: chrono::Utc::now().timestamp() + 7200,
            })
        })
    }
}

impl WahooApiPort for DevWahooOAuthClient {
    fn list_workouts(
        &self,
        _access_token: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        Box::pin(async move {
            Ok(WahooWorkoutList {
                workouts: vec![sample_workout()],
                total: 1,
                page,
                per_page,
                order: Some("descending".to_string()),
                sort: Some("starts".to_string()),
            })
        })
    }

    fn get_workout(
        &self,
        _access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async move {
            let workout = sample_workout();
            if workout.id == workout_id {
                Ok(workout)
            } else {
                Err(WahooError::NotFound)
            }
        })
    }

    fn get_workout_summary(
        &self,
        _access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async move {
            let workout = sample_workout();
            if workout.id == workout_id {
                Ok(workout.workout_summary)
            } else {
                Ok(None)
            }
        })
    }

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        let file_url = file_url.to_string();
        Box::pin(async move {
            if file_url.trim().is_empty() {
                return Err(WahooError::External(
                    "invalid dev Wahoo file url".to_string(),
                ));
            }

            Ok(b"dev-wahoo-fit".to_vec())
        })
    }
}

fn sample_workout() -> WahooWorkout {
    WahooWorkout {
        id: 56_519,
        starts: "2023-11-14T08:00:00.000Z".to_string(),
        minutes: Some(60),
        name: Some("Dev Wahoo Ride".to_string()),
        plan_id: None,
        plan_ids: Vec::new(),
        route_id: None,
        workout_token: Some("dev-workout-token".to_string()),
        workout_type_id: Some(0),
        workout_summary: Some(WahooWorkoutSummary {
            id: 8_297,
            name: Some("Dev Wahoo Ride".to_string()),
            ascent_meters: Some(450.0),
            cadence_avg_rpm: Some(50.0),
            calories: Some(1_500.0),
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
                url: "https://example.test/dev-wahoo.fit".to_string(),
            }),
            created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        }),
        created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
    }
}
