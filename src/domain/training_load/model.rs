#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingLoadError {
    Repository(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingLoadDashboardRange {
    Last90Days,
    Season,
    AllTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingLoadTsbZone {
    FreshnessPeak,
    OptimalTraining,
    HighRisk,
}

impl std::fmt::Display for TrainingLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TrainingLoadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FtpSource {
    Settings,
    Provider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FtpHistoryEntry {
    pub user_id: String,
    pub effective_from_date: String,
    pub ftp_watts: i32,
    pub source: FtpSource,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingLoadSnapshotRange {
    pub oldest: String,
    pub newest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingLoadDailySnapshot {
    pub user_id: String,
    pub date: String,
    pub daily_tss: Option<i32>,
    pub rolling_tss_7d: Option<f64>,
    pub rolling_tss_28d: Option<f64>,
    pub ctl: Option<f64>,
    pub atl: Option<f64>,
    pub tsb: Option<f64>,
    pub average_if_28d: Option<f64>,
    pub average_ef_28d: Option<f64>,
    pub ftp_effective_watts: Option<i32>,
    pub ftp_source: Option<FtpSource>,
    pub recomputed_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingLoadDashboardPoint {
    pub date: String,
    pub daily_tss: Option<i32>,
    pub ctl: Option<f64>,
    pub atl: Option<f64>,
    pub tsb: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingLoadDashboardSummary {
    pub current_ctl: Option<f64>,
    pub current_atl: Option<f64>,
    pub current_tsb: Option<f64>,
    pub ftp_watts: Option<i32>,
    pub average_if_28d: Option<f64>,
    pub average_ef_28d: Option<f64>,
    pub load_delta_ctl_14d: Option<f64>,
    pub tsb_zone: TrainingLoadTsbZone,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingLoadDashboardReport {
    pub range: TrainingLoadDashboardRange,
    pub window_start: String,
    pub window_end: String,
    pub has_training_load: bool,
    pub summary: TrainingLoadDashboardSummary,
    pub points: Vec<TrainingLoadDashboardPoint>,
}
