use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::llm::LlmProvider;
use crate::domain::settings::{
    validation, AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, BoxFuture,
    CyclingSettings, IntervalsConfig, SettingsError, UserSettings, UserSettingsRepository, Weekday,
};

#[derive(Clone)]
pub struct MongoUserSettingsRepository {
    collection: Collection<SettingsDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SettingsDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    user_id: String,
    ai_agents: AiAgentsDocument,
    intervals: IntervalsDocument,
    options: OptionsDocument,
    #[serde(default = "default_availability_document")]
    availability: AvailabilityDocument,
    cycling: CyclingDocument,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct AiAgentsDocument {
    openai_api_key: Option<String>,
    gemini_api_key: Option<String>,
    openrouter_api_key: Option<String>,
    selected_provider: Option<String>,
    selected_model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct IntervalsDocument {
    api_key: Option<String>,
    athlete_id: Option<String>,
    #[serde(default)]
    connected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct OptionsDocument {
    #[serde(default)]
    analyze_without_heart_rate: bool,
}

#[derive(Clone, Deserialize, Serialize, Default)]
struct CyclingDocument {
    full_name: Option<String>,
    age: Option<u32>,
    height_cm: Option<u32>,
    weight_kg: Option<f64>,
    ftp_watts: Option<u32>,
    hr_max_bpm: Option<u32>,
    vo2_max: Option<f64>,
    athlete_prompt: Option<String>,
    medications: Option<String>,
    athlete_notes: Option<String>,
    last_zone_update_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct AvailabilityDocument {
    #[serde(default)]
    configured: bool,
    #[serde(default = "default_availability_day_documents")]
    days: Vec<AvailabilityDayDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AvailabilityDayDocument {
    weekday: String,
    available: bool,
    max_duration_minutes: Option<u16>,
}

fn default_availability_document() -> AvailabilityDocument {
    AvailabilityDocument {
        configured: false,
        days: default_availability_day_documents(),
    }
}

fn default_availability_day_documents() -> Vec<AvailabilityDayDocument> {
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
        .into_iter()
        .map(|weekday| AvailabilityDayDocument {
            weekday: weekday.to_string(),
            available: false,
            max_duration_minutes: None,
        })
        .collect()
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

impl MongoUserSettingsRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("user_settings"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), SettingsError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("user_settings_user_id_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|e| SettingsError::Repository(e.to_string()))?;
        Ok(())
    }
}

impl UserSettingsRepository for MongoUserSettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let doc = collection
                .find_one(doc! { "user_id": &user_id })
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(doc.map(map_document_to_domain))
        })
    }

    fn upsert(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let collection = self.collection.clone();
        let user_id = settings.user_id.clone();
        let doc = map_domain_to_document(&settings);
        Box::pin(async move {
            collection
                .replace_one(doc! { "user_id": &user_id }, &doc)
                .upsert(true)
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
        updated_at: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .update_one(
                    doc! { "user_id": &user_id },
                    doc! {
                        "$set": {
                            "ai_agents.openai_api_key": &ai_agents.openai_api_key,
                            "ai_agents.gemini_api_key": &ai_agents.gemini_api_key,
                            "ai_agents.openrouter_api_key": &ai_agents.openrouter_api_key,
                            "ai_agents.selected_provider": ai_agents.selected_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.selected_model": &ai_agents.selected_model,
                            "updated_at_epoch_seconds": updated_at,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
        updated_at: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .update_one(
                    doc! { "user_id": &user_id },
                    doc! {
                        "$set": {
                            "intervals.api_key": &intervals.api_key,
                            "intervals.athlete_id": &intervals.athlete_id,
                            "intervals.connected": intervals.connected,
                            "updated_at_epoch_seconds": updated_at,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
        updated_at: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .update_one(
                    doc! { "user_id": &user_id },
                    doc! {
                        "$set": {
                            "options.analyze_without_heart_rate": options.analyze_without_heart_rate,
                            "updated_at_epoch_seconds": updated_at,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
        updated_at: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let availability_document = map_domain_availability_to_document(&availability);
            collection
                .update_one(
                    doc! { "user_id": &user_id },
                    doc! {
                        "$set": {
                            "availability.configured": availability_document.configured,
                            "availability.days": mongodb::bson::to_bson(&availability_document.days)
                                .map_err(|e| SettingsError::Repository(e.to_string()))?,
                            "updated_at_epoch_seconds": updated_at,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
        updated_at: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let cycling_document = map_domain_cycling_to_document(&cycling);
            collection
                .update_one(
                    doc! { "user_id": &user_id },
                    doc! {
                        "$set": {
                            "cycling.full_name": &cycling_document.full_name,
                            "cycling.age": cycling_document.age,
                            "cycling.height_cm": cycling_document.height_cm,
                            "cycling.weight_kg": cycling_document.weight_kg,
                            "cycling.ftp_watts": cycling_document.ftp_watts,
                            "cycling.hr_max_bpm": cycling_document.hr_max_bpm,
                            "cycling.vo2_max": cycling_document.vo2_max,
                            "cycling.athlete_prompt": &cycling_document.athlete_prompt,
                            "cycling.medications": &cycling_document.medications,
                            "cycling.athlete_notes": &cycling_document.athlete_notes,
                            "cycling.last_zone_update_epoch_seconds": cycling_document.last_zone_update_epoch_seconds,
                            "updated_at_epoch_seconds": updated_at,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }
}

fn map_document_to_domain(doc: SettingsDocument) -> UserSettings {
    UserSettings {
        user_id: doc.user_id,
        ai_agents: AiAgentsConfig {
            openai_api_key: doc.ai_agents.openai_api_key,
            gemini_api_key: doc.ai_agents.gemini_api_key,
            openrouter_api_key: doc.ai_agents.openrouter_api_key,
            selected_provider: doc
                .ai_agents
                .selected_provider
                .as_deref()
                .and_then(LlmProvider::parse),
            selected_model: doc.ai_agents.selected_model,
        },
        intervals: IntervalsConfig {
            api_key: doc.intervals.api_key,
            athlete_id: doc.intervals.athlete_id,
            connected: doc.intervals.connected,
        },
        options: AnalysisOptions {
            analyze_without_heart_rate: doc.options.analyze_without_heart_rate,
        },
        availability: map_document_availability_to_domain(doc.availability),
        cycling: map_document_cycling_to_domain(doc.cycling),
        created_at_epoch_seconds: doc.created_at_epoch_seconds,
        updated_at_epoch_seconds: doc.updated_at_epoch_seconds,
    }
}

fn map_domain_to_document(settings: &UserSettings) -> SettingsDocument {
    SettingsDocument {
        id: None,
        user_id: settings.user_id.clone(),
        ai_agents: AiAgentsDocument {
            openai_api_key: settings.ai_agents.openai_api_key.clone(),
            gemini_api_key: settings.ai_agents.gemini_api_key.clone(),
            openrouter_api_key: settings.ai_agents.openrouter_api_key.clone(),
            selected_provider: settings
                .ai_agents
                .selected_provider
                .as_ref()
                .map(|provider| provider.as_str().to_string()),
            selected_model: settings.ai_agents.selected_model.clone(),
        },
        intervals: IntervalsDocument {
            api_key: settings.intervals.api_key.clone(),
            athlete_id: settings.intervals.athlete_id.clone(),
            connected: settings.intervals.connected,
        },
        options: OptionsDocument {
            analyze_without_heart_rate: settings.options.analyze_without_heart_rate,
        },
        availability: map_domain_availability_to_document(&settings.availability),
        cycling: map_domain_cycling_to_document(&settings.cycling),
        created_at_epoch_seconds: settings.created_at_epoch_seconds,
        updated_at_epoch_seconds: settings.updated_at_epoch_seconds,
    }
}

fn map_document_cycling_to_domain(cycling: CyclingDocument) -> CyclingSettings {
    CyclingSettings {
        full_name: cycling.full_name,
        age: cycling.age,
        height_cm: cycling.height_cm,
        weight_kg: cycling.weight_kg,
        ftp_watts: cycling.ftp_watts,
        hr_max_bpm: cycling.hr_max_bpm,
        vo2_max: cycling.vo2_max,
        athlete_prompt: cycling.athlete_prompt,
        medications: cycling.medications,
        athlete_notes: cycling.athlete_notes,
        last_zone_update_epoch_seconds: cycling.last_zone_update_epoch_seconds,
    }
}

fn map_domain_cycling_to_document(cycling: &CyclingSettings) -> CyclingDocument {
    CyclingDocument {
        full_name: cycling.full_name.clone(),
        age: cycling.age,
        height_cm: cycling.height_cm,
        weight_kg: cycling.weight_kg,
        ftp_watts: cycling.ftp_watts,
        hr_max_bpm: cycling.hr_max_bpm,
        vo2_max: cycling.vo2_max,
        athlete_prompt: cycling.athlete_prompt.clone(),
        medications: cycling.medications.clone(),
        athlete_notes: cycling.athlete_notes.clone(),
        last_zone_update_epoch_seconds: cycling.last_zone_update_epoch_seconds,
    }
}

fn map_document_availability_to_domain(document: AvailabilityDocument) -> AvailabilitySettings {
    let has_complete_explicit_week = has_complete_explicit_week(&document.days);
    let repaired_days = repair_availability_days(document.days);

    match validation::validate_availability(AvailabilitySettings {
        configured: document.configured && has_complete_explicit_week,
        days: repaired_days,
    }) {
        Ok(availability) => availability,
        Err(error) => {
            tracing::warn!(error = %error, "falling back to default availability after unrecoverable settings document");
            AvailabilitySettings::default()
        }
    }
}

fn repair_availability_days(days: Vec<AvailabilityDayDocument>) -> Vec<AvailabilityDay> {
    use std::collections::BTreeMap;

    let mut repaired = BTreeMap::<Weekday, AvailabilityDay>::new();

    for day in days {
        let weekday = day.weekday.trim().to_lowercase();
        let Some(weekday) = Weekday::parse(&weekday) else {
            continue;
        };

        repaired.insert(
            weekday,
            AvailabilityDay {
                weekday,
                available: day.available
                    && day
                        .max_duration_minutes
                        .is_some_and(validation::is_allowed_availability_duration),
                max_duration_minutes: if day.available
                    && day
                        .max_duration_minutes
                        .is_some_and(validation::is_allowed_availability_duration)
                {
                    day.max_duration_minutes
                } else {
                    None
                },
            },
        );
    }

    Weekday::ALL
        .into_iter()
        .map(|weekday| {
            repaired.remove(&weekday).unwrap_or(AvailabilityDay {
                weekday,
                available: false,
                max_duration_minutes: None,
            })
        })
        .collect()
}

fn has_complete_explicit_week(days: &[AvailabilityDayDocument]) -> bool {
    let normalized_weekdays = days
        .iter()
        .map(|day| day.weekday.trim().to_lowercase())
        .collect::<Vec<_>>();
    let distinct_valid_weekdays = days
        .iter()
        .map(|day| day.weekday.trim().to_lowercase())
        .filter_map(|weekday| Weekday::parse(&weekday))
        .collect::<std::collections::BTreeSet<_>>();

    distinct_valid_weekdays.len() == 7 && normalized_weekdays.len() == 7
}

fn map_domain_availability_to_document(
    availability: &AvailabilitySettings,
) -> AvailabilityDocument {
    AvailabilityDocument {
        configured: availability.configured,
        days: availability
            .days
            .iter()
            .map(|day| AvailabilityDayDocument {
                weekday: day.weekday.as_str().to_string(),
                available: day.available,
                max_duration_minutes: day.max_duration_minutes,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use mongodb::{
        bson::{doc, oid::ObjectId},
        Client,
    };

    use super::{
        default_availability_document, map_document_availability_to_domain,
        map_domain_availability_to_document, AiAgentsDocument, MongoUserSettingsRepository,
        OptionsDocument, SettingsDocument,
    };
    use crate::domain::settings::{
        AvailabilityDay, AvailabilitySettings, UserSettingsRepository, Weekday,
    };

    #[test]
    fn settings_document_deserializes_missing_availability_with_full_week_default() {
        let document = serde_json::json!({
            "user_id": "user-1",
            "ai_agents": {},
            "intervals": {},
            "options": {},
            "cycling": {},
            "created_at_epoch_seconds": 1,
            "updated_at_epoch_seconds": 1
        });

        let parsed: SettingsDocument = serde_json::from_value(document).unwrap();

        assert!(!parsed.availability.configured);
        assert_eq!(parsed.availability.days.len(), 7);
        assert!(parsed.availability.days.iter().all(|day| !day.available));
    }

    #[test]
    fn map_document_availability_to_domain_falls_back_for_legacy_empty_days() {
        let availability = map_document_availability_to_domain(super::AvailabilityDocument {
            configured: false,
            days: Vec::new(),
        });

        assert!(!availability.configured);
        assert_eq!(availability.days.len(), 7);
        assert!(availability.days.iter().all(|day| !day.available));
    }

    #[test]
    fn map_document_availability_to_domain_repairs_case_and_missing_days() {
        let availability = map_document_availability_to_domain(super::AvailabilityDocument {
            configured: true,
            days: vec![
                super::AvailabilityDayDocument {
                    weekday: " MON ".to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: Some(90),
                },
            ],
        });

        assert!(!availability.is_configured());
        assert_eq!(availability.days.len(), 7);
        assert_eq!(availability.days[0].weekday, Weekday::Mon);
        assert_eq!(availability.days[0].max_duration_minutes, Some(60));
        assert_eq!(availability.days[1].weekday, Weekday::Tue);
        assert_eq!(availability.days[1].max_duration_minutes, None);
        assert!(availability.days[2..].iter().all(|day| !day.available));
    }

    #[test]
    fn map_document_availability_to_domain_sanitizes_invalid_duration_without_resetting_week() {
        let availability = map_document_availability_to_domain(super::AvailabilityDocument {
            configured: true,
            days: vec![
                super::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(45),
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Wed.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(90),
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Thu.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Fri.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Sat.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Sun.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

        assert!(availability.is_configured());
        assert_eq!(availability.days[0].weekday, Weekday::Mon);
        assert!(!availability.days[0].available);
        assert_eq!(availability.days[0].max_duration_minutes, None);
        assert_eq!(availability.days[2].weekday, Weekday::Wed);
        assert!(availability.days[2].available);
        assert_eq!(availability.days[2].max_duration_minutes, Some(90));
    }

    #[test]
    fn map_document_availability_to_domain_keeps_partial_legacy_week_unconfigured() {
        let availability = map_document_availability_to_domain(super::AvailabilityDocument {
            configured: true,
            days: vec![
                super::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

        assert!(!availability.configured);
        assert!(!availability.is_configured());
        assert!(availability.days[0].available);
        assert_eq!(availability.days[0].max_duration_minutes, Some(60));
    }

    #[test]
    fn map_document_availability_to_domain_treats_duplicate_weekdays_as_unconfigured() {
        let availability = map_document_availability_to_domain(super::AvailabilityDocument {
            configured: true,
            days: vec![
                super::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Wed.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Thu.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Fri.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Sat.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::AvailabilityDayDocument {
                    weekday: Weekday::Sun.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

        assert!(!availability.configured);
        assert!(!availability.is_configured());
    }

    #[tokio::test]
    async fn update_availability_updates_only_target_user_document() {
        let client = test_mongo_client().await;
        let database_name = unique_test_database_name("user-settings-availability");
        let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
        let collection = client
            .database(&database_name)
            .collection::<SettingsDocument>("user_settings");

        let user_1_id = "user-availability-target";
        let user_2_id = "user-availability-untouched";

        collection
            .insert_many([
                build_settings_document(user_1_id, 10),
                build_settings_document(user_2_id, 20),
            ])
            .await
            .unwrap();

        let availability = AvailabilitySettings {
            configured: true,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: true,
                    max_duration_minutes: Some(60),
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: true,
                    max_duration_minutes: Some(90),
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: true,
                    max_duration_minutes: Some(120),
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        };
        let updated_at = 123_456;

        repository
            .update_availability(user_1_id, availability.clone(), updated_at)
            .await
            .unwrap();

        let updated = collection
            .find_one(doc! { "user_id": user_1_id })
            .await
            .unwrap()
            .unwrap();
        let untouched = collection
            .find_one(doc! { "user_id": user_2_id })
            .await
            .unwrap()
            .unwrap();

        let expected_availability = map_domain_availability_to_document(&availability);

        assert_eq!(
            updated.availability.configured,
            expected_availability.configured
        );
        assert_eq!(
            updated.availability.days.len(),
            expected_availability.days.len()
        );
        assert_eq!(
            updated.availability.days[0].weekday,
            expected_availability.days[0].weekday
        );
        assert_eq!(
            updated.availability.days[0].available,
            expected_availability.days[0].available
        );
        assert_eq!(
            updated.availability.days[0].max_duration_minutes,
            expected_availability.days[0].max_duration_minutes
        );
        assert_eq!(
            updated.availability.days[2].weekday,
            expected_availability.days[2].weekday
        );
        assert_eq!(
            updated.availability.days[2].available,
            expected_availability.days[2].available
        );
        assert_eq!(
            updated.availability.days[2].max_duration_minutes,
            expected_availability.days[2].max_duration_minutes
        );
        assert_eq!(updated.updated_at_epoch_seconds, updated_at);

        let default_availability = default_availability_document();

        assert_eq!(untouched.user_id, user_2_id);
        assert_eq!(untouched.updated_at_epoch_seconds, 20);
        assert_eq!(
            untouched.availability.configured,
            default_availability.configured
        );
        assert_eq!(
            untouched.availability.days.len(),
            default_availability.days.len()
        );
        assert!(untouched.availability.days.iter().all(|day| !day.available));
        assert_eq!(untouched.availability.days[0].weekday, "mon");
        assert_eq!(untouched.availability.days[6].weekday, "sun");

        client.database(&database_name).drop().await.unwrap();
    }

    fn build_settings_document(user_id: &str, updated_at_epoch_seconds: i64) -> SettingsDocument {
        SettingsDocument {
            id: Some(ObjectId::new()),
            user_id: user_id.to_string(),
            ai_agents: AiAgentsDocument::default(),
            intervals: super::IntervalsDocument::default(),
            options: OptionsDocument::default(),
            availability: default_availability_document(),
            cycling: super::CyclingDocument::default(),
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds,
        }
    }

    async fn test_mongo_client() -> Client {
        Client::with_uri_str("mongodb://localhost:27017")
            .await
            .expect("test mongo client should be created")
    }

    fn unique_test_database_name(prefix: &str) -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{prefix}-{unique}")
    }
}
