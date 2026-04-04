use serde::Serialize;

#[derive(Serialize)]
pub(super) struct AthleteSummaryDto {
    pub exists: bool,
    pub stale: bool,
    #[serde(rename = "summaryText")]
    pub summary_text: Option<String>,
    #[serde(rename = "generatedAtEpochSeconds")]
    pub generated_at_epoch_seconds: Option<i64>,
    #[serde(rename = "updatedAtEpochSeconds")]
    pub updated_at_epoch_seconds: Option<i64>,
}
