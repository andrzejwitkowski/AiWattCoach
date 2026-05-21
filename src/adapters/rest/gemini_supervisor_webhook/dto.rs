use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiSupervisorWebhookRequest {
    #[serde(rename = "type")]
    pub(crate) event_type: String,
    pub(crate) data: GeminiSupervisorWebhookData,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiSupervisorWebhookData {
    pub(crate) id: String,
}
