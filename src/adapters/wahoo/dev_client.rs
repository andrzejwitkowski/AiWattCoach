use crate::domain::wahoo::{
    BoxFuture, WahooApiPort, WahooCreatePlan, WahooCreateWorkout, WahooError, WahooFileReference,
    WahooOAuthPort, WahooPlan, WahooToken, WahooUpdatePlan, WahooUpdateWorkout, WahooUser,
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
    fn list_plans(
        &self,
        _access_token: &str,
        external_id: Option<&str>,
    ) -> BoxFuture<Result<Vec<WahooPlan>, WahooError>> {
        let external_id = external_id.map(ToString::to_string);
        Box::pin(async move {
            let plan = sample_plan();
            Ok(match external_id {
                Some(external_id) if external_id != plan.external_id => Vec::new(),
                _ => vec![plan],
            })
        })
    }

    fn create_plan(
        &self,
        _access_token: &str,
        request: WahooCreatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async move {
            Ok(WahooPlan {
                id: 5001,
                external_id: request.external_id,
                provider_updated_at: Some(request.provider_updated_at),
                filename: request.filename,
                name: Some("Dev Outdoor Plan".to_string()),
                description: Some("Dev Wahoo plan".to_string()),
                created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
                updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
            })
        })
    }

    fn update_plan(
        &self,
        _access_token: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> BoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async move {
            Ok(WahooPlan {
                id: plan_id,
                external_id: "dev-planned-workout-id".to_string(),
                provider_updated_at: Some(request.provider_updated_at),
                filename: request.filename,
                name: Some("Dev Outdoor Plan".to_string()),
                description: Some("Updated dev Wahoo plan".to_string()),
                created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
                updated_at: Some("2023-11-14T09:00:00.000Z".to_string()),
            })
        })
    }

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

    fn get_authenticated_user(
        &self,
        _access_token: &str,
    ) -> BoxFuture<Result<WahooUser, WahooError>> {
        Box::pin(async { Ok(WahooUser { id: 60_462 }) })
    }

    fn create_workout(
        &self,
        _access_token: &str,
        request: WahooCreateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async move {
            let mut workout = sample_workout();
            workout.id = 60_001;
            workout.name = Some(request.name);
            workout.workout_token = Some(request.workout_token);
            workout.workout_type_id = Some(request.workout_type_id);
            workout.starts = request.starts;
            workout.minutes = Some(request.minutes);
            workout.plan_id = request.plan_id;
            Ok(workout)
        })
    }

    fn update_workout(
        &self,
        _access_token: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async move {
            let mut workout = sample_workout();
            workout.id = workout_id;
            if let Some(name) = request.name {
                workout.name = Some(name);
            }
            if let Some(workout_token) = request.workout_token {
                workout.workout_token = Some(workout_token);
            }
            if let Some(workout_type_id) = request.workout_type_id {
                workout.workout_type_id = Some(workout_type_id);
            }
            if let Some(starts) = request.starts {
                workout.starts = starts;
            }
            if let Some(minutes) = request.minutes {
                workout.minutes = Some(minutes);
            }
            workout.plan_id = request.plan_id.or(workout.plan_id);
            Ok(workout)
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

fn sample_plan() -> WahooPlan {
    WahooPlan {
        id: 5_001,
        external_id: "dev-planned-workout-id".to_string(),
        provider_updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        filename: Some("plan.json".to_string()),
        name: Some("Dev Outdoor Plan".to_string()),
        description: Some("Dev Wahoo plan".to_string()),
        created_at: Some("2023-11-14T08:00:00.000Z".to_string()),
        updated_at: Some("2023-11-14T08:00:00.000Z".to_string()),
    }
}

fn sample_workout() -> WahooWorkout {
    WahooWorkout {
        id: 56_519,
        starts: "2023-11-14T08:00:00.000Z".to_string(),
        minutes: Some(60),
        name: Some("Dev Wahoo Ride".to_string()),
        plan_id: Some(5_001),
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
