use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::settings::{
    AiAgentsConfig, AnalysisOptions, BoxFuture, CyclingSettings, IntervalsConfig, SettingsError,
    UserSettings, UserSettingsRepository,
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
    cycling: CyclingDocument,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct AiAgentsDocument {
    openai_api_key: Option<String>,
    gemini_api_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct IntervalsDocument {
    api_key: Option<String>,
    athlete_id: Option<String>,
    connected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct OptionsDocument {
    analyze_without_heart_rate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
struct CyclingDocument {
    full_name: Option<String>,
    age: Option<u32>,
    height_cm: Option<u32>,
    weight_kg: Option<f64>,
    ftp_watts: Option<u32>,
    hr_max_bpm: Option<u32>,
    vo2_max: Option<f64>,
    last_zone_update_epoch_seconds: Option<i64>,
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

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
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
                            "cycling.full_name": &cycling.full_name,
                            "cycling.age": cycling.age,
                            "cycling.height_cm": cycling.height_cm,
                            "cycling.weight_kg": cycling.weight_kg,
                            "cycling.ftp_watts": cycling.ftp_watts,
                            "cycling.hr_max_bpm": cycling.hr_max_bpm,
                            "cycling.vo2_max": cycling.vo2_max,
                            "cycling.last_zone_update_epoch_seconds": cycling.last_zone_update_epoch_seconds,
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
        },
        intervals: IntervalsConfig {
            api_key: doc.intervals.api_key,
            athlete_id: doc.intervals.athlete_id,
            connected: doc.intervals.connected,
        },
        options: AnalysisOptions {
            analyze_without_heart_rate: doc.options.analyze_without_heart_rate,
        },
        cycling: CyclingSettings {
            full_name: doc.cycling.full_name,
            age: doc.cycling.age,
            height_cm: doc.cycling.height_cm,
            weight_kg: doc.cycling.weight_kg,
            ftp_watts: doc.cycling.ftp_watts,
            hr_max_bpm: doc.cycling.hr_max_bpm,
            vo2_max: doc.cycling.vo2_max,
            last_zone_update_epoch_seconds: doc.cycling.last_zone_update_epoch_seconds,
        },
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
        },
        intervals: IntervalsDocument {
            api_key: settings.intervals.api_key.clone(),
            athlete_id: settings.intervals.athlete_id.clone(),
            connected: settings.intervals.connected,
        },
        options: OptionsDocument {
            analyze_without_heart_rate: settings.options.analyze_without_heart_rate,
        },
        cycling: CyclingDocument {
            full_name: settings.cycling.full_name.clone(),
            age: settings.cycling.age,
            height_cm: settings.cycling.height_cm,
            weight_kg: settings.cycling.weight_kg,
            ftp_watts: settings.cycling.ftp_watts,
            hr_max_bpm: settings.cycling.hr_max_bpm,
            vo2_max: settings.cycling.vo2_max,
            last_zone_update_epoch_seconds: settings.cycling.last_zone_update_epoch_seconds,
        },
        created_at_epoch_seconds: settings.created_at_epoch_seconds,
        updated_at_epoch_seconds: settings.updated_at_epoch_seconds,
    }
}
