use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum OptionalStringInput {
    #[default]
    Missing,
    Null,
    Value(String),
}

impl<'de> Deserialize<'de> for OptionalStringInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<String>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Serialize)]
pub(super) struct UserSettingsDto {
    #[serde(rename = "aiAgents")]
    pub(super) ai_agents: AiAgentsDto,
    pub(super) intervals: IntervalsDto,
    pub(super) options: OptionsDto,
    pub(super) cycling: CyclingDto,
}

#[derive(Serialize)]
pub(super) struct AiAgentsDto {
    #[serde(rename = "openaiApiKey")]
    pub(super) openai_api_key: Option<String>,
    #[serde(rename = "openaiApiKeySet")]
    pub(super) openai_api_key_set: bool,
    #[serde(rename = "geminiApiKey")]
    pub(super) gemini_api_key: Option<String>,
    #[serde(rename = "geminiApiKeySet")]
    pub(super) gemini_api_key_set: bool,
    #[serde(rename = "openrouterApiKey")]
    pub(super) openrouter_api_key: Option<String>,
    #[serde(rename = "openrouterApiKeySet")]
    pub(super) openrouter_api_key_set: bool,
    #[serde(rename = "selectedProvider")]
    pub(super) selected_provider: Option<String>,
    #[serde(rename = "selectedModel")]
    pub(super) selected_model: Option<String>,
}

#[derive(Serialize)]
pub(super) struct IntervalsDto {
    #[serde(rename = "apiKey")]
    pub(super) api_key: Option<String>,
    #[serde(rename = "apiKeySet")]
    pub(super) api_key_set: bool,
    #[serde(rename = "athleteId")]
    pub(super) athlete_id: Option<String>,
    pub(super) connected: bool,
}

#[derive(Serialize)]
pub(super) struct OptionsDto {
    #[serde(rename = "analyzeWithoutHeartRate")]
    pub(super) analyze_without_heart_rate: bool,
}

#[derive(Serialize)]
pub(super) struct CyclingDto {
    #[serde(rename = "fullName")]
    pub(super) full_name: Option<String>,
    pub(super) age: Option<u32>,
    #[serde(rename = "heightCm")]
    pub(super) height_cm: Option<u32>,
    #[serde(rename = "weightKg")]
    pub(super) weight_kg: Option<f64>,
    #[serde(rename = "ftpWatts")]
    pub(super) ftp_watts: Option<u32>,
    #[serde(rename = "hrMaxBpm")]
    pub(super) hr_max_bpm: Option<u32>,
    #[serde(rename = "vo2Max")]
    pub(super) vo2_max: Option<f64>,
    #[serde(rename = "lastZoneUpdateEpochSeconds")]
    pub(super) last_zone_update_epoch_seconds: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateAiAgentsRequest {
    #[serde(default, rename = "openaiApiKey")]
    pub(super) openai_api_key: OptionalStringInput,
    #[serde(default, rename = "geminiApiKey")]
    pub(super) gemini_api_key: OptionalStringInput,
    #[serde(default, rename = "openrouterApiKey")]
    pub(super) openrouter_api_key: OptionalStringInput,
    #[serde(default, rename = "selectedProvider")]
    pub(super) selected_provider: OptionalStringInput,
    #[serde(default, rename = "selectedModel")]
    pub(super) selected_model: OptionalStringInput,
}

#[derive(Deserialize)]
pub(crate) struct UpdateIntervalsRequest {
    #[serde(rename = "apiKey")]
    pub(super) api_key: Option<String>,
    #[serde(rename = "athleteId")]
    pub(super) athlete_id: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateOptionsRequest {
    #[serde(rename = "analyzeWithoutHeartRate")]
    pub(super) analyze_without_heart_rate: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateCyclingRequest {
    #[serde(rename = "fullName")]
    pub(super) full_name: Option<String>,
    pub(super) age: Option<u32>,
    #[serde(rename = "heightCm")]
    pub(super) height_cm: Option<u32>,
    #[serde(rename = "weightKg")]
    pub(super) weight_kg: Option<f64>,
    #[serde(rename = "ftpWatts")]
    pub(super) ftp_watts: Option<u32>,
    #[serde(rename = "hrMaxBpm")]
    pub(super) hr_max_bpm: Option<u32>,
    #[serde(rename = "vo2Max")]
    pub(super) vo2_max: Option<f64>,
}

#[derive(Deserialize)]
pub(crate) struct TestIntervalsConnectionRequest {
    #[serde(rename = "apiKey")]
    pub(super) api_key: Option<String>,
    #[serde(rename = "athleteId")]
    pub(super) athlete_id: Option<String>,
}

#[derive(Serialize)]
pub(super) struct TestIntervalsConnectionResponse {
    pub(super) connected: bool,
    pub(super) message: String,
    #[serde(rename = "usedSavedApiKey")]
    pub(super) used_saved_api_key: bool,
    #[serde(rename = "usedSavedAthleteId")]
    pub(super) used_saved_athlete_id: bool,
    #[serde(rename = "persistedStatusUpdated")]
    pub(super) persisted_status_updated: bool,
}

#[derive(Serialize)]
pub(super) struct TestAiAgentsConnectionResponse {
    pub(super) connected: bool,
    pub(super) message: String,
    #[serde(rename = "usedSavedApiKey")]
    pub(super) used_saved_api_key: bool,
    #[serde(rename = "usedSavedProvider")]
    pub(super) used_saved_provider: bool,
    #[serde(rename = "usedSavedModel")]
    pub(super) used_saved_model: bool,
}

#[derive(Serialize)]
pub(super) struct ValidationMessageResponse {
    pub(super) message: String,
}

pub(super) fn test_connection_response(
    connected: bool,
    message: &str,
    used_saved_api_key: bool,
    used_saved_athlete_id: bool,
) -> TestIntervalsConnectionResponse {
    TestIntervalsConnectionResponse {
        connected,
        message: message.to_string(),
        used_saved_api_key,
        used_saved_athlete_id,
        persisted_status_updated: false,
    }
}

pub(super) fn test_ai_agents_connection_response(
    connected: bool,
    message: &str,
    used_saved_api_key: bool,
    used_saved_provider: bool,
    used_saved_model: bool,
) -> TestAiAgentsConnectionResponse {
    TestAiAgentsConnectionResponse {
        connected,
        message: message.to_string(),
        used_saved_api_key,
        used_saved_provider,
        used_saved_model,
    }
}

pub(super) fn validation_message_response(message: &str) -> ValidationMessageResponse {
    ValidationMessageResponse {
        message: message.to_string(),
    }
}
