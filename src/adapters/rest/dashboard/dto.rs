use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct TrainingLoadDashboardQuery {
    pub range: String,
}

#[derive(Serialize)]
pub(crate) struct TrainingLoadDashboardPointDto {
    pub date: String,
    #[serde(rename = "dailyTss")]
    pub daily_tss: Option<i32>,
    #[serde(rename = "currentCtl")]
    pub ctl: Option<f64>,
    #[serde(rename = "currentAtl")]
    pub atl: Option<f64>,
    #[serde(rename = "currentTsb")]
    pub tsb: Option<f64>,
}

#[derive(Serialize)]
pub(crate) struct TrainingLoadDashboardSummaryDto {
    #[serde(rename = "currentCtl")]
    pub current_ctl: Option<f64>,
    #[serde(rename = "currentAtl")]
    pub current_atl: Option<f64>,
    #[serde(rename = "currentTsb")]
    pub current_tsb: Option<f64>,
    #[serde(rename = "ftpWatts")]
    pub ftp_watts: Option<i32>,
    #[serde(rename = "averageIf28d")]
    pub average_if_28d: Option<f64>,
    #[serde(rename = "averageEf28d")]
    pub average_ef_28d: Option<f64>,
    #[serde(rename = "loadDeltaCtl14d")]
    pub load_delta_ctl_14d: Option<f64>,
    #[serde(rename = "tsbZone")]
    pub tsb_zone: &'static str,
}

#[derive(Serialize)]
pub(crate) struct TrainingLoadDashboardResponseDto {
    pub range: &'static str,
    #[serde(rename = "windowStart")]
    pub window_start: String,
    #[serde(rename = "windowEnd")]
    pub window_end: String,
    #[serde(rename = "hasTrainingLoad")]
    pub has_training_load: bool,
    pub summary: TrainingLoadDashboardSummaryDto,
    pub points: Vec<TrainingLoadDashboardPointDto>,
}
