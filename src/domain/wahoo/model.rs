#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooError {
    Unauthenticated,
    InvalidConnectState,
    NotConnected,
    NotFound,
    Repository(String),
    External(String),
}

impl std::fmt::Display for WahooError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::InvalidConnectState => write!(f, "Wahoo connect state is invalid or expired"),
            Self::NotConnected => write!(f, "Wahoo account is not connected"),
            Self::NotFound => write!(f, "Wahoo resource not found"),
            Self::Repository(message) | Self::External(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WahooError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooFileReference {
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooUser {
    pub id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooPlan {
    pub id: i64,
    pub external_id: String,
    pub provider_updated_at: Option<String>,
    pub filename: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooCreatePlan {
    pub file_base64: String,
    pub filename: Option<String>,
    pub external_id: String,
    pub provider_updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooUpdatePlan {
    pub file_base64: String,
    pub filename: Option<String>,
    pub provider_updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooCreateWorkout {
    pub name: String,
    pub workout_token: String,
    pub workout_type_id: i64,
    pub starts: String,
    pub minutes: i32,
    pub plan_id: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WahooUpdateWorkout {
    pub name: Option<String>,
    pub workout_token: Option<String>,
    pub workout_type_id: Option<i64>,
    pub starts: Option<String>,
    pub minutes: Option<i32>,
    pub plan_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WahooWorkoutSummary {
    pub id: i64,
    pub name: Option<String>,
    pub ascent_meters: Option<f64>,
    pub cadence_avg_rpm: Option<f64>,
    pub calories: Option<f64>,
    pub distance_meters: Option<f64>,
    pub duration_active_seconds: Option<f64>,
    pub duration_paused_seconds: Option<f64>,
    pub duration_total_seconds: Option<f64>,
    pub heart_rate_avg_bpm: Option<f64>,
    pub normalized_power_watts: Option<f64>,
    pub training_stress_score: Option<f64>,
    pub average_power_watts: Option<f64>,
    pub speed_avg_mps: Option<f64>,
    pub total_work_joules: Option<f64>,
    pub time_zone: Option<String>,
    pub manual: bool,
    pub edited: bool,
    pub fitness_app_id: Option<i64>,
    pub file: Option<WahooFileReference>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WahooWorkout {
    pub id: i64,
    pub starts: String,
    pub minutes: Option<i32>,
    pub name: Option<String>,
    pub plan_id: Option<i64>,
    pub plan_ids: Vec<i64>,
    pub route_id: Option<i64>,
    pub workout_token: Option<String>,
    pub workout_type_id: Option<i64>,
    pub workout_summary: Option<WahooWorkoutSummary>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WahooWorkoutList {
    pub workouts: Vec<WahooWorkout>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub order: Option<String>,
    pub sort: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooConnectState {
    pub id: String,
    pub user_id: String,
    pub return_to: Option<String>,
    pub expires_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
}

impl WahooConnectState {
    pub fn new(
        id: String,
        user_id: String,
        return_to: Option<String>,
        expires_at_epoch_seconds: i64,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            return_to,
            expires_at_epoch_seconds,
            created_at_epoch_seconds,
        }
    }

    pub fn is_expired(&self, now_epoch_seconds: i64) -> bool {
        self.expires_at_epoch_seconds <= now_epoch_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooAuthStart {
    pub state: String,
    pub redirect_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooAuthExchange {
    pub redirect_to: String,
    pub token: WahooToken,
}
