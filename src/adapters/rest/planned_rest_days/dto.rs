use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ListPlannedRestDaysQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct PlannedRestDayPath {
    pub planned_rest_day_id: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct UpsertPlannedRestDayRequest {
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub(super) struct PlannedRestDayDto {
    #[serde(rename = "plannedRestDayId")]
    pub planned_rest_day_id: String,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
    #[serde(rename = "createdAtEpochSeconds")]
    pub created_at_epoch_seconds: i64,
    #[serde(rename = "updatedAtEpochSeconds")]
    pub updated_at_epoch_seconds: i64,
}
