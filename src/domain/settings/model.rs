use crate::domain::llm::LlmProvider;

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsError {
    Unauthenticated,
    Repository(String),
    Validation(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SettingsError {}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AiAgentsConfig {
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub selected_provider: Option<LlmProvider>,
    pub selected_model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct IntervalsConfig {
    pub api_key: Option<String>,
    pub athlete_id: Option<String>,
    pub connected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AnalysisOptions {
    pub analyze_without_heart_rate: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CyclingSettings {
    pub full_name: Option<String>,
    pub age: Option<u32>,
    pub height_cm: Option<u32>,
    pub weight_kg: Option<f64>,
    pub ftp_watts: Option<u32>,
    pub hr_max_bpm: Option<u32>,
    pub vo2_max: Option<f64>,
    pub athlete_prompt: Option<String>,
    pub medications: Option<String>,
    pub athlete_notes: Option<String>,
    pub last_zone_update_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserSettings {
    pub user_id: String,
    pub ai_agents: AiAgentsConfig,
    pub intervals: IntervalsConfig,
    pub options: AnalysisOptions,
    pub cycling: CyclingSettings,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl UserSettings {
    pub fn new_defaults(user_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            user_id,
            ai_agents: AiAgentsConfig::default(),
            intervals: IntervalsConfig::default(),
            options: AnalysisOptions::default(),
            cycling: CyclingSettings::default(),
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}

pub fn mask_sensitive(value: &Option<String>) -> Option<String> {
    value.as_ref().map(|v| {
        let char_count = v.chars().count();
        if char_count <= 4 {
            "***".to_string()
        } else {
            let last_four: String = v.chars().skip(char_count - 4).collect();
            format!("***...{last_four}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_sensitive_none() {
        assert_eq!(mask_sensitive(&None), None);
    }

    #[test]
    fn test_mask_sensitive_short_string() {
        assert_eq!(
            mask_sensitive(&Some("abc".to_string())),
            Some("***".to_string())
        );
    }

    #[test]
    fn test_mask_sensitive_exact_four_chars() {
        assert_eq!(
            mask_sensitive(&Some("abcd".to_string())),
            Some("***".to_string())
        );
    }

    #[test]
    fn test_mask_sensitive_long_string() {
        assert_eq!(
            mask_sensitive(&Some("sk-abc123xyz789".to_string())),
            Some("***...z789".to_string())
        );
    }

    #[test]
    fn test_new_defaults() {
        let user_id = "user123".to_string();
        let now = 1700000000i64;
        let settings = UserSettings::new_defaults(user_id.clone(), now);

        assert_eq!(settings.user_id, user_id);
        assert_eq!(settings.ai_agents, AiAgentsConfig::default());
        assert_eq!(settings.intervals, IntervalsConfig::default());
        assert_eq!(settings.options, AnalysisOptions::default());
        assert_eq!(settings.cycling, CyclingSettings::default());
        assert_eq!(settings.created_at_epoch_seconds, now);
        assert_eq!(settings.updated_at_epoch_seconds, now);
    }

    #[test]
    fn test_user_settings_partial_eq() {
        let now = 1700000000i64;
        let settings1 = UserSettings::new_defaults("user1".to_string(), now);
        let settings2 = UserSettings::new_defaults("user1".to_string(), now);
        let settings3 = UserSettings::new_defaults("user2".to_string(), now);

        assert_eq!(settings1, settings2);
        assert_ne!(settings1, settings3);
    }
}
