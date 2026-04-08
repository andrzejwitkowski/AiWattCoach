use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::calendar::{
    BoxFuture, CalendarError, PlannedWorkoutSyncRecord, PlannedWorkoutSyncRepository,
    PlannedWorkoutSyncStatus,
};
use crate::domain::intervals::DateRange;

#[derive(Clone)]
pub struct MongoPlannedWorkoutSyncRepository {
    collection: Collection<PlannedWorkoutSyncDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PlannedWorkoutSyncDocument {
    user_id: String,
    operation_key: String,
    date: String,
    source_workout_id: String,
    intervals_event_id: Option<i64>,
    status: String,
    synced_payload_hash: Option<String>,
    last_error: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
    last_synced_at_epoch_seconds: Option<i64>,
}

impl MongoPlannedWorkoutSyncRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("planned_workout_syncs"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CalendarError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "operation_key": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_syncs_user_operation_date_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_syncs_user_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "intervals_event_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_syncs_user_intervals_event".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| CalendarError::Internal(error.to_string()))?;

        Ok(())
    }
}

impl PlannedWorkoutSyncRepository for MongoPlannedWorkoutSyncRepository {
    fn find_by_user_id_and_projection(
        &self,
        user_id: &str,
        operation_key: &str,
        date: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutSyncRecord>, CalendarError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let date = date.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "operation_key": &operation_key,
                    "date": &date,
                })
                .await
                .map_err(|error| CalendarError::Internal(error.to_string()))?
                .map(map_document_to_record)
                .transpose()
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedWorkoutSyncRecord>, CalendarError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &range.oldest,
                        "$lte": &range.newest,
                    },
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| CalendarError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| CalendarError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_record)
                .collect()
        })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutSyncRecord, CalendarError>> {
        let collection = self.collection.clone();
        let document = map_record_to_document(&record);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "operation_key": &document.operation_key,
                        "date": &document.date,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| CalendarError::Internal(error.to_string()))?;
            Ok(record)
        })
    }
}

fn map_record_to_document(record: &PlannedWorkoutSyncRecord) -> PlannedWorkoutSyncDocument {
    PlannedWorkoutSyncDocument {
        user_id: record.user_id.clone(),
        operation_key: record.operation_key.clone(),
        date: record.date.clone(),
        source_workout_id: record.source_workout_id.clone(),
        intervals_event_id: record.intervals_event_id,
        status: record.status.as_str().to_string(),
        synced_payload_hash: record.synced_payload_hash.clone(),
        last_error: record.last_error.clone(),
        created_at_epoch_seconds: record.created_at_epoch_seconds,
        updated_at_epoch_seconds: record.updated_at_epoch_seconds,
        last_synced_at_epoch_seconds: record.last_synced_at_epoch_seconds,
    }
}

fn map_document_to_record(
    document: PlannedWorkoutSyncDocument,
) -> Result<PlannedWorkoutSyncRecord, CalendarError> {
    Ok(PlannedWorkoutSyncRecord {
        user_id: document.user_id,
        operation_key: document.operation_key,
        date: document.date,
        source_workout_id: document.source_workout_id,
        intervals_event_id: document.intervals_event_id,
        status: map_status(document.status.as_str())?,
        synced_payload_hash: document.synced_payload_hash,
        last_error: document.last_error,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        last_synced_at_epoch_seconds: document.last_synced_at_epoch_seconds,
    })
}

fn map_status(value: &str) -> Result<PlannedWorkoutSyncStatus, CalendarError> {
    match value {
        "unsynced" => Ok(PlannedWorkoutSyncStatus::Unsynced),
        "pending" => Ok(PlannedWorkoutSyncStatus::Pending),
        "synced" => Ok(PlannedWorkoutSyncStatus::Synced),
        "modified" => Ok(PlannedWorkoutSyncStatus::Modified),
        "failed" => Ok(PlannedWorkoutSyncStatus::Failed),
        other => Err(CalendarError::Internal(format!(
            "unknown planned workout sync status: {other}"
        ))),
    }
}
