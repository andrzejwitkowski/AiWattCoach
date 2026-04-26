use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct WahooTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize)]
pub struct WahooFileReferenceResponse {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WahooPlanResponse {
    pub id: i64,
    pub external_id: Option<String>,
    pub provider_updated_at: Option<String>,
    pub filename: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WahooWorkoutSummaryResponse {
    pub id: i64,
    pub name: Option<String>,
    pub ascent_accum: Option<String>,
    pub cadence_avg: Option<String>,
    pub calories_accum: Option<String>,
    pub distance_accum: Option<String>,
    pub duration_active_accum: Option<String>,
    pub duration_paused_accum: Option<String>,
    pub duration_total_accum: Option<String>,
    pub heart_rate_avg: Option<String>,
    pub power_bike_np_last: Option<String>,
    pub power_bike_tss_last: Option<String>,
    pub power_avg: Option<String>,
    pub speed_avg: Option<String>,
    pub work_accum: Option<String>,
    pub time_zone: Option<String>,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub edited: bool,
    pub fitness_app_id: Option<i64>,
    pub file: Option<WahooFileReferenceResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WahooWorkoutResponse {
    pub id: i64,
    pub starts: String,
    pub minutes: Option<i32>,
    pub name: Option<String>,
    pub plan_id: Option<i64>,
    #[serde(default)]
    pub plan_ids: Vec<i64>,
    pub route_id: Option<i64>,
    pub workout_token: Option<String>,
    pub workout_type_id: Option<i64>,
    pub workout_summary: Option<WahooWorkoutSummaryResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WahooWorkoutListResponse {
    #[serde(default)]
    pub workouts: Vec<WahooWorkoutResponse>,
    pub total: Option<usize>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub order: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WahooCreatePlanRequest {
    pub plan: WahooCreatePlanRequestBody,
}

#[derive(Debug, Serialize)]
pub struct WahooCreatePlanRequestBody {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub external_id: String,
    pub provider_updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct WahooUpdatePlanRequest {
    pub plan: WahooUpdatePlanRequestBody,
}

#[derive(Debug, Serialize)]
pub struct WahooUpdatePlanRequestBody {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub provider_updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct WahooCreateWorkoutRequest {
    pub workout: WahooCreateWorkoutRequestBody,
}

#[derive(Debug, Serialize)]
pub struct WahooCreateWorkoutRequestBody {
    pub name: String,
    pub workout_token: String,
    pub workout_type_id: i64,
    pub starts: String,
    pub minutes: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WahooUpdateWorkoutRequest {
    pub workout: WahooUpdateWorkoutRequestBody,
}

#[derive(Debug, Default, Serialize)]
pub struct WahooUpdateWorkoutRequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_type_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<i64>,
}
