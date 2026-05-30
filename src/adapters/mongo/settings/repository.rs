use mongodb::Collection;

use super::{
    bootstrap::{
        backfill_wahoo_user_id_impl, list_intervals_poll_bootstrap_users_impl,
        list_wahoo_user_id_backfill_candidates_impl,
    },
    documents::SettingsDocument,
    mapping::{
        map_document_to_domain, map_domain_availability_to_document,
        map_domain_cycling_to_document, map_domain_to_document,
    },
};
use crate::adapters::mongo::time::optional_epoch_seconds_to_bson_datetime;
use crate::domain::settings::{
    AnalysisOptions, AvailabilitySettings, BoxFuture, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsRepository, WahooUserIdBackfillCandidate,
};
use futures::TryStreamExt;
use mongodb::bson::doc;

#[derive(Clone)]
pub struct MongoUserSettingsRepository {
    pub(super) collection: Collection<SettingsDocument>,
}

impl MongoUserSettingsRepository {
    pub async fn list_intervals_poll_bootstrap_users(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<super::IntervalsPollBootstrapUser>, SettingsError> {
        list_intervals_poll_bootstrap_users_impl(&self.collection, user_ids).await
    }

    pub async fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> Result<Vec<WahooUserIdBackfillCandidate>, SettingsError> {
        list_wahoo_user_id_backfill_candidates_impl(&self.collection).await
    }

    pub async fn backfill_wahoo_user_id(
        &self,
        user_id: &str,
        wahoo_user_id: i64,
        updated_at_epoch_seconds: i64,
    ) -> Result<(), SettingsError> {
        backfill_wahoo_user_id_impl(
            &self.collection,
            user_id,
            wahoo_user_id,
            updated_at_epoch_seconds,
        )
        .await
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
        ai_agents: crate::domain::settings::AiAgentsConfig,
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
                            "ai_agents.selected_provider": ai_agents.selected_provider.as_ref().map(|provider| provider.as_str()),
                            "ai_agents.selected_model": &ai_agents.selected_model,
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
