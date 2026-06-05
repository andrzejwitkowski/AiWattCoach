use serde::Serialize;

#[derive(Serialize)]
pub struct MesoCycleWindowDto {
    #[serde(rename = "mesoStart")]
    pub meso_start: String,
    #[serde(rename = "mesoEnd")]
    pub meso_end: String,
    #[serde(rename = "aiCoachLastDate")]
    pub ai_coach_last_date: Option<String>,
}

#[derive(Serialize)]
pub struct MesoCycleOperationDto {
    #[serde(rename = "operationKey")]
    pub operation_key: String,
    pub status: String,
    #[serde(rename = "mesoStart")]
    pub meso_start: Option<String>,
    #[serde(rename = "mesoEnd")]
    pub meso_end: Option<String>,
    #[serde(rename = "failureMessage")]
    pub failure_message: Option<String>,
    #[serde(rename = "updatedAtEpochSeconds")]
    pub updated_at_epoch_seconds: i64,
}

#[derive(Serialize)]
pub struct MesoCycleStatusDto {
    pub window: Option<MesoCycleWindowDto>,
    #[serde(rename = "hasPendingGeneration")]
    pub has_pending_generation: bool,
    #[serde(rename = "latestOperation")]
    pub latest_operation: Option<MesoCycleOperationDto>,
}

#[derive(Serialize)]
pub struct MesoCycleCalendarDayDto {
    pub date: String,
    #[serde(rename = "restDay")]
    pub rest_day: bool,
    #[serde(rename = "restDayReason")]
    pub rest_day_reason: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "rawWorkoutDoc")]
    pub raw_workout_doc: Option<String>,
    #[serde(rename = "overlapStatus")]
    pub overlap_status: String,
}
