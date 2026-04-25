use serde::Deserialize;

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
