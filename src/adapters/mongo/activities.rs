use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    Activity, ActivityDeduplicationIdentity, ActivityRepositoryPort, BoxFuture, DateRange,
    IntervalsError,
};

#[derive(Clone)]
pub struct MongoActivityRepository {
    collection: Collection<ActivityDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivityDocument {
    user_id: String,
    activity_id: String,
    start_date_local: String,
    external_id_normalized: Option<String>,
    fallback_identity_v1: Option<String>,
    payload: Activity,
}

impl MongoActivityRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("intervals_activities"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IntervalsError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "activity_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_activity_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "start_date_local": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_start_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "external_id_normalized": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_external_id".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "fallback_identity_v1": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_fallback_identity".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?;
        Ok(())
    }
}

impl ActivityRepositoryPort for MongoActivityRepository {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let dedupe_identity = ActivityDeduplicationIdentity::from_activity(&activity);
            let document = ActivityDocument {
                user_id: user_id.clone(),
                activity_id: activity.id.clone(),
                start_date_local: activity.start_date_local.clone(),
                external_id_normalized: dedupe_identity.normalized_external_id,
                fallback_identity_v1: dedupe_identity.fallback_identity,
                payload: activity.clone(),
            };
            collection
                .replace_one(
                    doc! { "user_id": &user_id, "activity_id": &document.activity_id },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = Vec::with_capacity(activities.len());
            for activity in activities {
                stored.push(repository.upsert(&user_id, activity).await?);
            }
            Ok(stored)
        })
    }

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let mut cursor = collection
                .find(doc! {
                    "user_id": &user_id,
                    "start_date_local": { "$gte": &range.oldest, "$lte": &range.newest }
                })
                .sort(doc! { "start_date_local": -1 })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;

            let mut activities = Vec::new();
            while cursor
                .advance()
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?
            {
                let document = cursor
                    .deserialize_current()
                    .map_err(|error| IntervalsError::Internal(error.to_string()))?;
                activities.push(document.payload);
            }
            Ok(activities)
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let result = collection
                .find_one(doc! { "user_id": &user_id, "activity_id": &activity_id })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(result.map(|document| document.payload))
        })
    }

    fn find_by_user_id_and_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            let result = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "external_id_normalized": &external_id,
                })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(result.map(|document| document.payload))
        })
    }

    fn find_by_user_id_and_fallback_identity(
        &self,
        user_id: &str,
        identity: &str,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let identity = identity.to_string();
        Box::pin(async move {
            let mut cursor = collection
                .find(doc! {
                    "user_id": &user_id,
                    "fallback_identity_v1": &identity,
                })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;

            let mut activities = Vec::new();
            while cursor
                .advance()
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?
            {
                let document = cursor
                    .deserialize_current()
                    .map_err(|error| IntervalsError::Internal(error.to_string()))?;
                activities.push(document.payload);
            }
            Ok(activities)
        })
    }

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "user_id": &user_id, "activity_id": &activity_id })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::{from_document, to_document};

    use super::ActivityDocument;
    use crate::domain::intervals::{
        Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
        ActivityStream,
    };

    #[test]
    fn activity_document_bson_round_trip_preserves_enriched_completed_fields() {
        let payload = enriched_activity();
        let document = ActivityDocument {
            user_id: "user-1".to_string(),
            activity_id: payload.id.clone(),
            start_date_local: payload.start_date_local.clone(),
            external_id_normalized: Some("external-i78".to_string()),
            fallback_identity_v1: Some("v1:2026-03-22T08:00|ride|3720|40200|false".to_string()),
            payload: payload.clone(),
        };

        let bson = to_document(&document).expect("serialize activity document");
        let restored: ActivityDocument =
            from_document(bson).expect("deserialize activity document");

        assert_eq!(restored.user_id, document.user_id);
        assert_eq!(restored.activity_id, document.activity_id);
        assert_eq!(restored.start_date_local, document.start_date_local);
        assert_eq!(
            restored.external_id_normalized,
            document.external_id_normalized
        );
        assert_eq!(restored.fallback_identity_v1, document.fallback_identity_v1);
        assert_eq!(restored.payload.metrics, payload.metrics);
        assert_eq!(
            restored.payload.details.intervals,
            payload.details.intervals
        );
        assert_eq!(
            restored.payload.details.interval_groups,
            payload.details.interval_groups
        );
        assert_eq!(restored.payload.details.streams, payload.details.streams);
        assert_eq!(restored.payload, payload);
    }

    fn enriched_activity() -> Activity {
        Activity {
            id: "i78".to_string(),
            athlete_id: Some("athlete-42".to_string()),
            start_date_local: "2026-03-22T08:00:00".to_string(),
            start_date: Some("2026-03-22T07:00:00Z".to_string()),
            name: Some("Completed Workout".to_string()),
            description: Some("structured ride".to_string()),
            activity_type: Some("Ride".to_string()),
            source: Some("STRAVA".to_string()),
            external_id: Some("external-i78".to_string()),
            device_name: Some("Garmin Edge".to_string()),
            distance_meters: Some(40200.0),
            moving_time_seconds: Some(3600),
            elapsed_time_seconds: Some(3720),
            total_elevation_gain_meters: Some(510.0),
            total_elevation_loss_meters: Some(505.0),
            average_speed_mps: Some(11.1),
            max_speed_mps: Some(16.4),
            average_heart_rate_bpm: Some(148),
            max_heart_rate_bpm: Some(175),
            average_cadence_rpm: Some(89.5),
            trainer: false,
            commute: false,
            race: false,
            has_heart_rate: true,
            stream_types: vec!["watts".to_string(), "heartrate".to_string()],
            tags: vec!["tempo".to_string()],
            metrics: ActivityMetrics {
                training_stress_score: Some(72),
                normalized_power_watts: Some(238),
                intensity_factor: Some(0.84),
                efficiency_factor: Some(1.28),
                variability_index: Some(1.04),
                average_power_watts: Some(228),
                ftp_watts: Some(283),
                total_work_joules: Some(820),
                calories: Some(690),
                trimp: Some(92.0),
                power_load: Some(72),
                heart_rate_load: Some(66),
                pace_load: None,
                strain_score: Some(13.7),
            },
            details: ActivityDetails {
                intervals: vec![ActivityInterval {
                    id: Some(1),
                    label: Some("Threshold".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("set-1".to_string()),
                    start_index: Some(10),
                    end_index: Some(20),
                    start_time_seconds: Some(300),
                    end_time_seconds: Some(600),
                    moving_time_seconds: Some(300),
                    elapsed_time_seconds: Some(300),
                    distance_meters: Some(2500.0),
                    average_power_watts: Some(285),
                    normalized_power_watts: Some(290),
                    training_stress_score: Some(18.5),
                    average_heart_rate_bpm: Some(168),
                    average_cadence_rpm: Some(94.0),
                    average_speed_mps: Some(8.2),
                    average_stride_meters: None,
                    zone: Some(4),
                }],
                interval_groups: vec![ActivityIntervalGroup {
                    id: "set-1".to_string(),
                    count: Some(3),
                    start_index: Some(10),
                    moving_time_seconds: Some(900),
                    elapsed_time_seconds: Some(900),
                    distance_meters: Some(7500.0),
                    average_power_watts: Some(280),
                    normalized_power_watts: Some(286),
                    training_stress_score: Some(55.5),
                    average_heart_rate_bpm: Some(165),
                    average_cadence_rpm: Some(92.0),
                    average_speed_mps: Some(8.0),
                    average_stride_meters: None,
                }],
                streams: vec![ActivityStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    data: Some(serde_json::json!([120, 250, 310])),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                }],
                interval_summary: vec!["tempo".to_string()],
                skyline_chart: vec!["z2".to_string(), "z4".to_string()],
                power_zone_times: Vec::new(),
                heart_rate_zone_times: vec![120, 240],
                pace_zone_times: vec![60],
                gap_zone_times: vec![90],
            },
        }
    }
}
