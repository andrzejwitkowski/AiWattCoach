use mongodb::{
    bson::{doc, to_document},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::error::is_duplicate_key_error;
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

const MAX_ACTIVITY_UPSERT_RETRIES: usize = 5;

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

    if activity_detail_richness(&incoming) > activity_detail_richness(&existing) {
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

fn build_activity_document(user_id: &str, activity: Activity) -> ActivityDocument {
    let dedupe_identity = ActivityDeduplicationIdentity::from_activity(&activity);

    ActivityDocument {
        user_id: user_id.to_string(),
        activity_id: activity.id.clone(),
        start_date_local: activity.start_date_local.clone(),
        external_id_normalized: dedupe_identity.normalized_external_id,
        fallback_identity_v1: dedupe_identity.fallback_identity,
        payload: activity,
    }
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
            for _ in 0..MAX_ACTIVITY_UPSERT_RETRIES {
                let existing_document = collection
                    .find_one(doc! { "user_id": &user_id, "activity_id": &activity.id })
                    .await
                    .map_err(|error| IntervalsError::Internal(error.to_string()))?;
                let merged_activity = merge_activity_for_storage(
                    existing_document
                        .as_ref()
                        .map(|document| document.payload.clone()),
                    activity.clone(),
                );
                let document = build_activity_document(&user_id, merged_activity.clone());

                if let Some(existing_document) = existing_document {
                    let filter = to_document(&existing_document)
                        .map_err(|error| IntervalsError::Internal(error.to_string()))?;
                    let result = collection
                        .replace_one(filter, &document)
                        .await
                        .map_err(|error| IntervalsError::Internal(error.to_string()))?;

                    if result.matched_count == 1 {
                        return Ok(merged_activity);
                    }

                    continue;
                }

                match collection.insert_one(&document).await {
                    Ok(_) => return Ok(merged_activity),
                    Err(error) if is_duplicate_key_error(&error) => continue,
                    Err(error) => return Err(IntervalsError::Internal(error.to_string())),
                }
            }

            Err(IntervalsError::Internal(
                "activity upsert conflicted too many times".to_string(),
            ))
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
#[path = "activities_tests.rs"]
mod tests;
