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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Self; 7] = [
        Self::Mon,
        Self::Tue,
        Self::Wed,
        Self::Thu,
        Self::Fri,
        Self::Sat,
        Self::Sun,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mon => "mon",
            Self::Tue => "tue",
            Self::Wed => "wed",
            Self::Thu => "thu",
            Self::Fri => "fri",
            Self::Sat => "sat",
            Self::Sun => "sun",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mon" => Some(Self::Mon),
            "tue" => Some(Self::Tue),
            "wed" => Some(Self::Wed),
            "thu" => Some(Self::Thu),
            "fri" => Some(Self::Fri),
            "sat" => Some(Self::Sat),
            "sun" => Some(Self::Sun),
            _ => None,
        }
    }
}

impl std::fmt::Display for Weekday {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for Weekday {
    fn default() -> Self {
        Self::Mon
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailabilityDay {
    pub weekday: Weekday,
    pub available: bool,
    pub max_duration_minutes: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailabilitySettings {
    pub configured: bool,
    pub days: Vec<AvailabilityDay>,
}

#[derive(Clone, PartialEq, Default)]
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

impl std::fmt::Debug for CyclingSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CyclingSettings")
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
            .finish()
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct UserSettings {
    pub user_id: String,
    pub ai_agents: AiAgentsConfig,
    pub intervals: IntervalsConfig,
    pub options: AnalysisOptions,
    pub availability: AvailabilitySettings,
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
            availability: AvailabilitySettings::default(),
            cycling: CyclingSettings::default(),
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}

impl AvailabilitySettings {
    pub fn is_configured(&self) -> bool {
        self.configured && self.days.len() == 7 && self.days.iter().any(|day| day.available)
    }

    pub fn from_days(days: Vec<AvailabilityDay>) -> Self {
        let ordered_days = order_availability_days(days);
        let configured = ordered_days.iter().any(|day| day.available);
        Self {
            configured,
            days: ordered_days,
        }
    }
}

impl Default for AvailabilitySettings {
    fn default() -> Self {
        Self {
            configured: false,
            days: default_availability_days(),
        }
    }
}

pub fn default_availability_days() -> Vec<AvailabilityDay> {
    Weekday::ALL
        .into_iter()
        .map(|weekday| AvailabilityDay {
            weekday,
            available: false,
            max_duration_minutes: None,
        })
        .collect()
}

fn order_availability_days(days: Vec<AvailabilityDay>) -> Vec<AvailabilityDay> {
    let mut by_weekday = days
        .into_iter()
        .map(|day| (day.weekday, day))
        .collect::<std::collections::BTreeMap<_, _>>();

    Weekday::ALL
        .into_iter()
        .filter_map(|weekday| by_weekday.remove(&weekday))
        .collect()
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

    #[test]
    fn cycling_settings_debug_redacts_sensitive_profile_fields() {
        let settings = CyclingSettings {
            athlete_prompt: Some("prompt details".to_string()),
            medications: Some("medication details".to_string()),
            athlete_notes: Some("note details".to_string()),
            ..CyclingSettings::default()
        };

        let debug_output = format!("{settings:?}");

        assert!(!debug_output.contains("prompt details"));
        assert!(!debug_output.contains("medication details"));
        assert!(!debug_output.contains("note details"));
        assert!(debug_output.contains("<redacted:"));
    }

    #[test]
    fn availability_is_not_configured_without_any_available_days() {
        let settings = AvailabilitySettings {
            configured: true,
            days: default_availability_days(),
        };

        assert!(!settings.is_configured());
    }

    #[test]
    fn availability_from_days_derives_configured_state() {
        let mut days = default_availability_days();
        days[0].available = true;
        days[0].max_duration_minutes = Some(60);

        let settings = AvailabilitySettings::from_days(days);

        assert!(settings.configured);
        assert!(settings.is_configured());
    }

    #[test]
    fn availability_from_days_orders_weekdays_canonically() {
        let settings = AvailabilitySettings::from_days(vec![
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: false,
                max_duration_minutes: None,
            },
        ]);

        assert_eq!(
            settings
                .days
                .iter()
                .map(|day| day.weekday.as_str())
                .collect::<Vec<_>>(),
            vec!["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        );
    }

    #[test]
    fn weekday_parse_round_trips_supported_values() {
        assert_eq!(Weekday::parse("mon"), Some(Weekday::Mon));
        assert_eq!(Weekday::parse("sun"), Some(Weekday::Sun));
        assert_eq!(Weekday::Mon.as_str(), "mon");
        assert_eq!(Weekday::Sun.to_string(), "sun");
        assert_eq!(Weekday::parse("MON"), None);
        assert_eq!(Weekday::parse("monday"), None);
    }
}
