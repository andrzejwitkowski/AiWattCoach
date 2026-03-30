use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    Activity, ActivityDeduplicationIdentity, ActivityDetails, ActivityMetrics,
    ActivityRepositoryPort, BoxFuture, DateRange, IntervalsError,
};

#[derive(Clone)]
pub struct MongoActivityRepository {
    collection: Collection<ActivityDocument>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

    pub async fn cleanup_legacy_time_streams(&self) -> Result<u64, IntervalsError> {
        let mut cursor = self
            .collection
            .find(doc! {
                "$or": [
                    { "payload.stream_types": { "$regex": "^time$", "$options": "i" } },
                    { "payload.details.streams.stream_type": { "$regex": "^time$", "$options": "i" } },
                ]
            })
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?;

        let mut cleaned_documents = 0;
        while cursor
            .advance()
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?
        {
            let document: ActivityDocument = cursor
                .deserialize_current()
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            let normalized_document = normalize_activity_document(document.clone());

            if normalized_document == document {
                continue;
            }

            self.collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "activity_id": &document.activity_id,
                    },
                    &normalized_document,
                )
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            cleaned_documents += 1;
        }

        Ok(cleaned_documents)
    }
}

fn merge_activity_for_storage(existing: Option<Activity>, incoming: Activity) -> Activity {
    let incoming = normalize_activity(incoming);
    let Some(existing) = existing else {
        return incoming;
    };
    let existing = normalize_activity(existing);

    if activity_detail_richness(&incoming) >= activity_detail_richness(&existing) {
        return incoming;
    }

    Activity {
        id: incoming.id,
        athlete_id: incoming.athlete_id.or(existing.athlete_id),
        start_date_local: incoming.start_date_local,
        start_date: incoming.start_date.or(existing.start_date),
        name: incoming.name.or(existing.name),
        description: incoming.description.or(existing.description),
        activity_type: incoming.activity_type.or(existing.activity_type),
        source: incoming.source.or(existing.source),
        external_id: incoming.external_id.or(existing.external_id),
        device_name: incoming.device_name.or(existing.device_name),
        distance_meters: incoming.distance_meters.or(existing.distance_meters),
        moving_time_seconds: incoming
            .moving_time_seconds
            .or(existing.moving_time_seconds),
        elapsed_time_seconds: incoming
            .elapsed_time_seconds
            .or(existing.elapsed_time_seconds),
        total_elevation_gain_meters: incoming
            .total_elevation_gain_meters
            .or(existing.total_elevation_gain_meters),
        total_elevation_loss_meters: incoming
            .total_elevation_loss_meters
            .or(existing.total_elevation_loss_meters),
        average_speed_mps: incoming.average_speed_mps.or(existing.average_speed_mps),
        max_speed_mps: incoming.max_speed_mps.or(existing.max_speed_mps),
        average_heart_rate_bpm: incoming
            .average_heart_rate_bpm
            .or(existing.average_heart_rate_bpm),
        max_heart_rate_bpm: incoming.max_heart_rate_bpm.or(existing.max_heart_rate_bpm),
        average_cadence_rpm: incoming
            .average_cadence_rpm
            .or(existing.average_cadence_rpm),
        trainer: incoming.trainer || existing.trainer,
        commute: incoming.commute || existing.commute,
        race: incoming.race || existing.race,
        has_heart_rate: incoming.has_heart_rate || existing.has_heart_rate,
        stream_types: prefer_non_empty(incoming.stream_types, existing.stream_types),
        tags: prefer_non_empty(incoming.tags, existing.tags),
        metrics: merge_activity_metrics(existing.metrics, incoming.metrics),
        details: merge_activity_details(existing.details, incoming.details),
        details_unavailable_reason: incoming
            .details_unavailable_reason
            .or(existing.details_unavailable_reason),
    }
}

fn activity_detail_richness(activity: &Activity) -> usize {
    usize::from(!activity.details.intervals.is_empty())
        + usize::from(!activity.details.interval_groups.is_empty())
        + usize::from(!activity.details.streams.is_empty())
        + usize::from(!activity.details.interval_summary.is_empty())
        + usize::from(!activity.details.skyline_chart.is_empty())
        + usize::from(!activity.details.power_zone_times.is_empty())
        + usize::from(!activity.details.heart_rate_zone_times.is_empty())
        + usize::from(!activity.details.pace_zone_times.is_empty())
        + usize::from(!activity.details.gap_zone_times.is_empty())
}

fn merge_activity_metrics(existing: ActivityMetrics, incoming: ActivityMetrics) -> ActivityMetrics {
    ActivityMetrics {
        training_stress_score: incoming
            .training_stress_score
            .or(existing.training_stress_score),
        normalized_power_watts: incoming
            .normalized_power_watts
            .or(existing.normalized_power_watts),
        intensity_factor: incoming.intensity_factor.or(existing.intensity_factor),
        efficiency_factor: incoming.efficiency_factor.or(existing.efficiency_factor),
        variability_index: incoming.variability_index.or(existing.variability_index),
        average_power_watts: incoming
            .average_power_watts
            .or(existing.average_power_watts),
        ftp_watts: incoming.ftp_watts.or(existing.ftp_watts),
        total_work_joules: incoming.total_work_joules.or(existing.total_work_joules),
        calories: incoming.calories.or(existing.calories),
        trimp: incoming.trimp.or(existing.trimp),
        power_load: incoming.power_load.or(existing.power_load),
        heart_rate_load: incoming.heart_rate_load.or(existing.heart_rate_load),
        pace_load: incoming.pace_load.or(existing.pace_load),
        strain_score: incoming.strain_score.or(existing.strain_score),
    }
}

fn merge_activity_details(existing: ActivityDetails, incoming: ActivityDetails) -> ActivityDetails {
    ActivityDetails {
        intervals: prefer_non_empty(incoming.intervals, existing.intervals),
        interval_groups: prefer_non_empty(incoming.interval_groups, existing.interval_groups),
        streams: prefer_non_empty(incoming.streams, existing.streams),
        interval_summary: prefer_non_empty(incoming.interval_summary, existing.interval_summary),
        skyline_chart: prefer_non_empty(incoming.skyline_chart, existing.skyline_chart),
        power_zone_times: prefer_non_empty(incoming.power_zone_times, existing.power_zone_times),
        heart_rate_zone_times: prefer_non_empty(
            incoming.heart_rate_zone_times,
            existing.heart_rate_zone_times,
        ),
        pace_zone_times: prefer_non_empty(incoming.pace_zone_times, existing.pace_zone_times),
        gap_zone_times: prefer_non_empty(incoming.gap_zone_times, existing.gap_zone_times),
    }
}

fn prefer_non_empty<T>(incoming: Vec<T>, existing: Vec<T>) -> Vec<T> {
    if incoming.is_empty() {
        existing
    } else {
        incoming
    }
}

fn normalize_activity(mut activity: Activity) -> Activity {
    activity
        .stream_types
        .retain(|stream_type| should_store_stream_type(stream_type));
    activity
        .details
        .streams
        .retain(|stream| should_store_stream_type(&stream.stream_type));
    activity
}

fn normalize_activity_document(mut document: ActivityDocument) -> ActivityDocument {
    let payload = normalize_activity(document.payload);
    let dedupe_identity = ActivityDeduplicationIdentity::from_activity(&payload);

    document.activity_id = payload.id.clone();
    document.start_date_local = payload.start_date_local.clone();
    document.external_id_normalized = dedupe_identity.normalized_external_id;
    document.fallback_identity_v1 = dedupe_identity.fallback_identity;
    document.payload = payload;
    document
}

fn should_store_stream_type(stream_type: &str) -> bool {
    !stream_type.eq_ignore_ascii_case("time")
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
            let existing = collection
                .find_one(doc! { "user_id": &user_id, "activity_id": &activity.id })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?
                .map(|document| document.payload);
            let activity = merge_activity_for_storage(existing, activity);
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
                activities.push(normalize_activity(document.payload));
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
            Ok(result.map(|document| normalize_activity(document.payload)))
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
            Ok(result.map(|document| normalize_activity(document.payload)))
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
                activities.push(normalize_activity(document.payload));
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
        Activity, ActivityDeduplicationIdentity, ActivityDetails, ActivityInterval,
        ActivityIntervalGroup, ActivityMetrics, ActivityStream,
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

    #[test]
    fn merge_sparse_activity_payload_preserves_existing_enriched_fields() {
        let existing = enriched_activity();
        let incoming = sparse_activity_stub(&existing.id);

        let merged = super::merge_activity_for_storage(Some(existing.clone()), incoming);

        assert_eq!(merged.id, existing.id);
        assert_eq!(merged.start_date_local, existing.start_date_local);
        assert_eq!(merged.name, existing.name);
        assert_eq!(merged.activity_type, existing.activity_type);
        assert_eq!(merged.distance_meters, existing.distance_meters);
        assert_eq!(merged.moving_time_seconds, existing.moving_time_seconds);
        assert_eq!(merged.metrics, existing.metrics);
        assert_eq!(merged.details, existing.details);
        assert_eq!(merged.stream_types, existing.stream_types);
        assert_eq!(merged.tags, existing.tags);
        assert!(merged.has_heart_rate);
    }

    #[test]
    fn merge_richer_incoming_activity_replaces_existing_payload() {
        let existing = enriched_activity();
        let mut incoming = enriched_activity();
        incoming.name = Some("Updated Completed Workout".to_string());
        incoming.details.streams = vec![ActivityStream {
            stream_type: "heartrate".to_string(),
            name: Some("Heart Rate".to_string()),
            data: Some(serde_json::json!([140, 150, 160])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        }];

        let merged = super::merge_activity_for_storage(Some(existing), incoming.clone());

        assert_eq!(merged, incoming);
    }

    #[test]
    fn merge_activity_for_storage_drops_time_streams() {
        let mut incoming = enriched_activity();
        incoming.stream_types = vec!["time".to_string(), "watts".to_string()];
        incoming.details.streams = vec![
            ActivityStream {
                stream_type: "time".to_string(),
                name: None,
                data: Some(serde_json::json!([0, 1, 2])),
                data2: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            },
            ActivityStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                data: Some(serde_json::json!([120, 250, 310])),
                data2: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            },
        ];

        let merged = super::merge_activity_for_storage(None, incoming);

        assert_eq!(merged.stream_types, vec!["watts".to_string()]);
        assert_eq!(merged.details.streams.len(), 1);
        assert_eq!(merged.details.streams[0].stream_type, "watts");
    }

    #[test]
    fn normalize_activity_document_drops_time_streams_and_refreshes_dedupe_fields() {
        let mut payload = enriched_activity();
        payload.external_id = Some("  EXTERNAL-I78  ".to_string());
        payload.stream_types = vec!["time".to_string(), "watts".to_string()];
        payload.details.streams = vec![
            ActivityStream {
                stream_type: "time".to_string(),
                name: None,
                data: Some(serde_json::json!([0, 1, 2])),
                data2: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            },
            ActivityStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                data: Some(serde_json::json!([120, 250, 310])),
                data2: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            },
        ];
        let normalized_payload = super::normalize_activity(payload.clone());
        let expected_dedupe_identity =
            ActivityDeduplicationIdentity::from_activity(&normalized_payload);

        let normalized_document = super::normalize_activity_document(ActivityDocument {
            user_id: "user-1".to_string(),
            activity_id: payload.id.clone(),
            start_date_local: payload.start_date_local.clone(),
            external_id_normalized: None,
            fallback_identity_v1: None,
            payload,
        });

        assert_eq!(normalized_document.payload.stream_types, vec!["watts"]);
        assert_eq!(normalized_document.payload.details.streams.len(), 1);
        assert_eq!(
            normalized_document.external_id_normalized,
            expected_dedupe_identity.normalized_external_id
        );
        assert_eq!(
            normalized_document.fallback_identity_v1,
            expected_dedupe_identity.fallback_identity
        );
    }

    #[test]
    fn normalize_activity_document_keeps_clean_document_unchanged() {
        let payload = enriched_activity();
        let dedupe_identity = ActivityDeduplicationIdentity::from_activity(&payload);
        let document = ActivityDocument {
            user_id: "user-1".to_string(),
            activity_id: payload.id.clone(),
            start_date_local: payload.start_date_local.clone(),
            external_id_normalized: dedupe_identity.normalized_external_id,
            fallback_identity_v1: dedupe_identity.fallback_identity,
            payload,
        };

        let normalized_document = super::normalize_activity_document(document.clone());

        assert_eq!(normalized_document, document);
    }

    fn sparse_activity_stub(id: &str) -> Activity {
        Activity {
            id: id.to_string(),
            athlete_id: None,
            start_date_local: "2026-03-22T08:00:00".to_string(),
            start_date: None,
            name: None,
            description: None,
            activity_type: None,
            source: Some("STRAVA".to_string()),
            external_id: None,
            device_name: None,
            distance_meters: None,
            moving_time_seconds: None,
            elapsed_time_seconds: None,
            total_elevation_gain_meters: None,
            total_elevation_loss_meters: None,
            average_speed_mps: None,
            max_speed_mps: None,
            average_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            average_cadence_rpm: None,
            trainer: false,
            commute: false,
            race: false,
            has_heart_rate: false,
            stream_types: Vec::new(),
            tags: Vec::new(),
            metrics: ActivityMetrics {
                training_stress_score: None,
                normalized_power_watts: None,
                intensity_factor: None,
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: None,
                ftp_watts: None,
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            details: ActivityDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: Vec::new(),
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            details_unavailable_reason: None,
        }
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
            details_unavailable_reason: None,
        }
    }
}
