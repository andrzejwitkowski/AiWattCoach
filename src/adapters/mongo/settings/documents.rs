use mongodb::bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct SettingsDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<ObjectId>,
    pub(super) user_id: String,
    pub(super) ai_agents: AiAgentsDocument,
    pub(super) intervals: IntervalsDocument,
    #[serde(default)]
    pub(super) wahoo: WahooDocument,
    pub(super) options: OptionsDocument,
    #[serde(default = "default_availability_document")]
    pub(super) availability: AvailabilityDocument,
    pub(super) cycling: CyclingDocument,
    pub(super) created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) created_at: Option<DateTime>,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) updated_at: Option<DateTime>,
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub(super) struct AiAgentsDocument {
    pub(super) openai_api_key: Option<String>,
    pub(super) gemini_api_key: Option<String>,
    pub(super) openrouter_api_key: Option<String>,
    pub(super) deepseek_api_key: Option<String>,
    pub(super) selected_provider: Option<String>,
    pub(super) selected_model: Option<String>,
}

impl std::fmt::Debug for AiAgentsDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiAgentsDocument")
            .field(
                "openai_api_key",
                &RedactedOptionalText(&self.openai_api_key),
            )
            .field(
                "gemini_api_key",
                &RedactedOptionalText(&self.gemini_api_key),
            )
            .field(
                "openrouter_api_key",
                &RedactedOptionalText(&self.openrouter_api_key),
            )
            .field(
                "deepseek_api_key",
                &RedactedOptionalText(&self.deepseek_api_key),
            )
            .field("selected_provider", &self.selected_provider)
            .field("selected_model", &self.selected_model)
            .finish()
    }
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub(super) struct IntervalsDocument {
    pub(super) api_key: Option<String>,
    pub(super) athlete_id: Option<String>,
    #[serde(default)]
    pub(super) connected: bool,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) updated_at: Option<DateTime>,
}

impl std::fmt::Debug for IntervalsDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntervalsDocument")
            .field("api_key", &RedactedOptionalText(&self.api_key))
            .field("athlete_id", &self.athlete_id)
            .field("connected", &self.connected)
            .field("updated_at_epoch_seconds", &self.updated_at_epoch_seconds)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub(super) struct WahooDocument {
    pub(super) access_token: Option<String>,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) expires_at: Option<DateTime>,
    #[serde(default)]
    pub(super) user_id: Option<i64>,
    #[serde(default)]
    pub(super) connected: bool,
    pub(super) updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) updated_at: Option<DateTime>,
}

impl std::fmt::Debug for WahooDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WahooDocument")
            .field("access_token", &RedactedOptionalText(&self.access_token))
            .field("refresh_token", &RedactedOptionalText(&self.refresh_token))
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .field("expires_at", &self.expires_at)
            .field("user_id", &self.user_id)
            .field("connected", &self.connected)
            .field("updated_at_epoch_seconds", &self.updated_at_epoch_seconds)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub(super) struct OptionsDocument {
    #[serde(default)]
    pub(super) analyze_without_heart_rate: bool,
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub(super) struct CyclingDocument {
    pub(super) full_name: Option<String>,
    pub(super) age: Option<u32>,
    pub(super) height_cm: Option<u32>,
    pub(super) weight_kg: Option<f64>,
    pub(super) ftp_watts: Option<u32>,
    pub(super) hr_max_bpm: Option<u32>,
    pub(super) vo2_max: Option<f64>,
    pub(super) athlete_prompt: Option<String>,
    pub(super) medications: Option<String>,
    pub(super) athlete_notes: Option<String>,
    pub(super) last_zone_update_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub(super) last_zone_update_at: Option<DateTime>,
}

impl std::fmt::Debug for CyclingDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CyclingDocument")
            .field("full_name", &self.full_name)
            .field("age", &self.age)
            .field("height_cm", &self.height_cm)
            .field("weight_kg", &self.weight_kg)
            .field("ftp_watts", &self.ftp_watts)
            .field("hr_max_bpm", &self.hr_max_bpm)
            .field("vo2_max", &self.vo2_max)
            .field(
                "athlete_prompt",
                &RedactedOptionalText(&self.athlete_prompt),
            )
            .field("medications", &RedactedOptionalText(&self.medications))
            .field("athlete_notes", &RedactedOptionalText(&self.athlete_notes))
            .field(
                "last_zone_update_epoch_seconds",
                &self.last_zone_update_epoch_seconds,
            )
            .field("last_zone_update_at", &self.last_zone_update_at)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub(super) struct AvailabilityDocument {
    #[serde(default)]
    pub(super) configured: bool,
    #[serde(default = "default_availability_day_documents")]
    pub(super) days: Vec<AvailabilityDayDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct AvailabilityDayDocument {
    pub(super) weekday: String,
    pub(super) available: bool,
    pub(super) max_duration_minutes: Option<u16>,
}

pub(super) fn default_availability_document() -> AvailabilityDocument {
    AvailabilityDocument {
        configured: false,
        days: default_availability_day_documents(),
    }
}

pub(super) fn default_availability_day_documents() -> Vec<AvailabilityDayDocument> {
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        .into_iter()
        .map(|weekday| AvailabilityDayDocument {
            weekday: weekday.to_string(),
            available: false,
            max_duration_minutes: None,
        })
        .collect()
}

struct RedactedOptionalText<'a>(&'a Option<String>);

impl std::fmt::Debug for RedactedOptionalText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "Some(<redacted:{} chars>)", value.chars().count()),
            None => write!(f, "None"),
        }
    }
}
