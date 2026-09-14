use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};

use super::super::time::optional_epoch_seconds_to_bson_datetime;
use super::document::{
    IntervalsPollBootstrapIntervalsDocument, IntervalsPollBootstrapUser,
    IntervalsPollBootstrapUserDocument, SettingsDocument, WahooPollBootstrapUserDocument,
};
use super::mapping::{
    map_document_to_domain, map_domain_availability_to_document, map_domain_cycling_to_document,
    map_domain_to_document,
};
use crate::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, BoxFuture, CyclingSettings,
    IntervalsConfig, SettingsError, UserSettings, UserSettingsRepository, WahooConfig,
    WahooUserIdBackfillCandidate,
};

#[derive(Clone)]
pub struct MongoUserSettingsRepository {
    collection: Collection<SettingsDocument>,
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
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("user_settings_user_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "wahoo.user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("user_settings_wahoo_user_id_unique".to_string())
                            .unique(true)
                            .partial_filter_expression(doc! {
                                "wahoo.user_id": { "$type": ["long", "int"] }
                            })
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|e| SettingsError::Repository(e.to_string()))?;
        Ok(())
    }

    pub async fn list_intervals_poll_bootstrap_users(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<IntervalsPollBootstrapUser>, SettingsError> {
        let poll_user_ids = user_ids
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let collection = self
            .collection
            .clone_with_type::<IntervalsPollBootstrapUserDocument>();
        let filter = build_intervals_poll_bootstrap_filter(user_ids);
        let documents = collection
            .find(filter)
            .projection(doc! {
                "_id": 0,
                "user_id": 1,
                "intervals": 1,
                "updated_at_epoch_seconds": 1,
            })
            .sort(doc! { "user_id": 1 })
            .await
            .map_err(|error| SettingsError::Repository(error.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| SettingsError::Repository(error.to_string()))?;

        Ok(documents
            .into_iter()
            .filter(|document| {
                if poll_user_ids.contains(&document.user_id) {
                    return true;
                }

                should_include_non_requested_bootstrap_user(document)
            })
            .map(|document| {
                let desired_active = document
                    .intervals
                    .as_ref()
                    .is_some_and(is_bootstrap_active_intervals);
                let intervals_updated_at_epoch_seconds = bootstrap_intervals_updated_at(&document);

                IntervalsPollBootstrapUser {
                    user_id: document.user_id,
                    desired_active,
                    intervals_updated_at_epoch_seconds,
                }
            })
            .collect())
    }

    pub async fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> Result<Vec<WahooUserIdBackfillCandidate>, SettingsError> {
        let collection = self
            .collection
            .clone_with_type::<WahooPollBootstrapUserDocument>();
        let documents = collection
            .find(doc! {
                "$and": [
                    { "wahoo.refresh_token": { "$type": "string", "$regex": "\\S" } },
                    { "wahoo.connected": { "$ne": false } },
                    { "wahoo.user_id": null },
                ]
            })
            .projection(doc! {
                "_id": 0,
                "user_id": 1,
                "wahoo": 1,
            })
            .sort(doc! { "user_id": 1 })
            .await
            .map_err(|error| SettingsError::Repository(error.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| SettingsError::Repository(error.to_string()))?;

        Ok(documents
            .into_iter()
            .filter_map(|document| {
                let wahoo = document.wahoo?;
                Some(WahooUserIdBackfillCandidate {
                    user_id: document.user_id,
                    wahoo: WahooConfig {
                        access_token: wahoo.access_token,
                        refresh_token: wahoo.refresh_token,
                        expires_at_epoch_seconds: None,
                        user_id: wahoo.user_id,
                        connected: wahoo.connected.unwrap_or(true),
                        updated_at_epoch_seconds: None,
                    },
                })
            })
            .collect())
    }

    pub async fn backfill_wahoo_user_id(
        &self,
        user_id: &str,
        wahoo_user_id: i64,
        updated_at_epoch_seconds: i64,
    ) -> Result<(), SettingsError> {
        let result = self
            .collection
            .update_one(
                doc! {
                    "user_id": user_id,
                    "$or": [
                        { "wahoo.user_id": null },
                        { "wahoo.user_id": wahoo_user_id },
                    ]
                },
                doc! {
                    "$set": {
                        "wahoo.user_id": wahoo_user_id,
                        "wahoo.updated_at_epoch_seconds": updated_at_epoch_seconds,
                        "wahoo.updated_at": optional_epoch_seconds_to_bson_datetime(
                            Some(updated_at_epoch_seconds),
                            "wahoo.updated_at"
                        ).map_err(SettingsError::Repository)?,
                    }
                },
            )
            .await
            .map_err(|error| SettingsError::Repository(error.to_string()))?;
        if result.matched_count == 0 {
            return Err(SettingsError::Repository(format!(
                "no settings document updated for user_id={user_id} wahoo_user_id={wahoo_user_id}",
            )));
        }
        Ok(())
    }
}

pub(super) fn has_non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn is_bootstrap_active_intervals(
    intervals: &IntervalsPollBootstrapIntervalsDocument,
) -> bool {
    has_non_empty(intervals.api_key.as_deref())
        && has_non_empty(intervals.athlete_id.as_deref())
        && intervals.connected != Some(false)
}

fn should_include_non_requested_bootstrap_user(
    document: &IntervalsPollBootstrapUserDocument,
) -> bool {
    document
        .intervals
        .as_ref()
        .is_some_and(is_bootstrap_active_intervals)
}

pub(super) fn bootstrap_intervals_updated_at(
    document: &IntervalsPollBootstrapUserDocument,
) -> Option<i64> {
    document
        .intervals
        .as_ref()
        .and_then(|intervals| intervals.updated_at_epoch_seconds)
        .or(document.updated_at_epoch_seconds)
}

pub(super) fn build_intervals_poll_bootstrap_filter(
    user_ids: &[String],
) -> mongodb::bson::Document {
    let mut filter_clauses = vec![doc! {
        "$and": [
            { "intervals.api_key": { "$type": "string", "$regex": "\\S" } },
            { "intervals.athlete_id": { "$type": "string", "$regex": "\\S" } },
            { "intervals.connected": { "$ne": false } },
        ]
    }];

    if !user_ids.is_empty() {
        filter_clauses.push(doc! { "user_id": { "$in": user_ids } });
    }

    doc! { "$or": filter_clauses }
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
            doc.map(map_document_to_domain).transpose()
        })
    }

    fn find_by_wahoo_user_id(
        &self,
        wahoo_user_id: i64,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let mut documents = collection
                .find(doc! { "wahoo.user_id": wahoo_user_id })
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            let first = documents
                .try_next()
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            let second = documents
                .try_next()
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            if second.is_some() {
                return Err(SettingsError::Repository(format!(
                    "multiple users are mapped to Wahoo user id {wahoo_user_id}"
                )));
            }

            first.map(map_document_to_domain).transpose()
        })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> BoxFuture<Result<Vec<WahooUserIdBackfillCandidate>, SettingsError>> {
        let repository = self.clone();
        Box::pin(async move { repository.list_wahoo_user_id_backfill_candidates().await })
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
                            "ai_agents.deepseek_api_key": &ai_agents.deepseek_api_key,
                            "ai_agents.zai_api_key": &ai_agents.zai_api_key,
                            "ai_agents.openai_compatible_api_key": &ai_agents.openai_compatible_api_key,
                            "ai_agents.openai_compatible_base_url": &ai_agents.openai_compatible_base_url,
                            "ai_agents.selected_provider": ai_agents.selected_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.selected_model": &ai_agents.selected_model,
                            "ai_agents.workout_chat_provider": ai_agents.workout_chat_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.workout_chat_model": &ai_agents.workout_chat_model,
                            "ai_agents.workout_planning_provider": ai_agents.workout_planning_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.workout_planning_model": &ai_agents.workout_planning_model,
                            "ai_agents.meso_cycle_provider": ai_agents.meso_cycle_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.meso_cycle_model": &ai_agents.meso_cycle_model,
                            "ai_agents.plan_quality_evaluator_provider": ai_agents.plan_quality_evaluator_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.plan_quality_evaluator_model": &ai_agents.plan_quality_evaluator_model,
                            "ai_agents.plan_quality_max_loops": ai_agents.plan_quality_max_loops,
                            "ai_agents.plan_quality_pass_score": ai_agents.plan_quality_pass_score,
                            "ai_agents.include_power_image": ai_agents.include_power_image,
                            "updated_at_epoch_seconds": updated_at,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "updated_at")
                                .map_err(SettingsError::Repository)?,
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
                            "intervals.updated_at_epoch_seconds": updated_at,
                            "intervals.updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "intervals.updated_at")
                                .map_err(SettingsError::Repository)?,
                            "updated_at_epoch_seconds": updated_at,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "updated_at")
                                .map_err(SettingsError::Repository)?,
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
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "updated_at")
                                .map_err(SettingsError::Repository)?,
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
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "updated_at")
                                .map_err(SettingsError::Repository)?,
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
                            "cycling.last_zone_update_at": optional_epoch_seconds_to_bson_datetime(
                                cycling_document.last_zone_update_epoch_seconds,
                                "cycling.last_zone_update_at",
                            )
                            .map_err(SettingsError::Repository)?,
                            "updated_at_epoch_seconds": updated_at,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at), "updated_at")
                                .map_err(SettingsError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(|e| SettingsError::Repository(e.to_string()))?;
            Ok(())
        })
    }
}
