use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{
    optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds,
    resolve_required_epoch_seconds,
};
use crate::domain::planned_workout_wahoo_syncs::{
    BoxFuture, PlannedWorkoutWahooSyncError, PlannedWorkoutWahooSyncRecord,
    PlannedWorkoutWahooSyncRepository, PlannedWorkoutWahooSyncStatus,
};

#[derive(Clone)]
pub struct MongoPlannedWorkoutWahooSyncRepository {
    collection: Collection<PlannedWorkoutWahooSyncDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PlannedWorkoutWahooSyncDocument {
    user_id: String,
    operation_key: String,
    date: String,
    planned_workout_id: String,
    source_workout_id: String,
    payload_hash: Option<String>,
    status: String,
    wahoo_plan_external_id: String,
    wahoo_plan_id: Option<i64>,
    wahoo_workout_id: Option<i64>,
    wahoo_workout_token: Option<String>,
    last_error: Option<String>,
    created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    updated_at: Option<DateTime>,
    last_synced_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_synced_at: Option<DateTime>,
}

impl MongoPlannedWorkoutWahooSyncRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("planned_workout_wahoo_syncs"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), PlannedWorkoutWahooSyncError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "planned_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_wahoo_syncs_user_planned_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "wahoo_plan_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_wahoo_syncs_user_plan".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "wahoo_workout_token": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_wahoo_syncs_user_token".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| PlannedWorkoutWahooSyncError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl PlannedWorkoutWahooSyncRepository for MongoPlannedWorkoutWahooSyncRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "planned_workout_id": &planned_workout_id,
                })
                .await
                .map_err(|error| PlannedWorkoutWahooSyncError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "wahoo_plan_id": wahoo_plan_id,
                })
                .await
                .map_err(|error| PlannedWorkoutWahooSyncError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "wahoo_workout_token": &wahoo_workout_token,
                })
                .await
                .map_err(|error| PlannedWorkoutWahooSyncError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutWahooSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutWahooSyncRecord, PlannedWorkoutWahooSyncError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&record);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "planned_workout_id": &document.planned_workout_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| PlannedWorkoutWahooSyncError::Repository(error.to_string()))?;
            Ok(record)
        })
    }
}

fn map_domain_to_document(
    record: &PlannedWorkoutWahooSyncRecord,
) -> PlannedWorkoutWahooSyncDocument {
    PlannedWorkoutWahooSyncDocument {
        user_id: record.user_id.clone(),
        operation_key: record.operation_key.clone(),
        date: record.date.clone(),
        planned_workout_id: record.planned_workout_id.clone(),
        source_workout_id: record.source_workout_id.clone(),
        payload_hash: record.payload_hash.clone(),
        status: record.status.as_str().to_string(),
        wahoo_plan_external_id: record.wahoo_plan_external_id.clone(),
        wahoo_plan_id: record.wahoo_plan_id,
        wahoo_workout_id: record.wahoo_workout_id,
        wahoo_workout_token: record.wahoo_workout_token.clone(),
        last_error: record.last_error.clone(),
        created_at_epoch_seconds: Some(record.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(record.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: Some(record.updated_at_epoch_seconds),
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(record.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
        last_synced_at_epoch_seconds: record.last_synced_at_epoch_seconds,
        last_synced_at: optional_epoch_seconds_to_bson_datetime(
            record.last_synced_at_epoch_seconds,
            "last_synced_at",
        )
        .expect("last_synced_at should fit BSON DateTime"),
    }
}

fn map_document_to_domain(
    document: PlannedWorkoutWahooSyncDocument,
) -> Result<PlannedWorkoutWahooSyncRecord, PlannedWorkoutWahooSyncError> {
    Ok(PlannedWorkoutWahooSyncRecord {
        user_id: document.user_id,
        operation_key: document.operation_key,
        date: document.date,
        planned_workout_id: document.planned_workout_id,
        source_workout_id: document.source_workout_id,
        payload_hash: document.payload_hash,
        status: map_status(&document.status)?,
        wahoo_plan_external_id: document.wahoo_plan_external_id,
        wahoo_plan_id: document.wahoo_plan_id,
        wahoo_workout_id: document.wahoo_workout_id,
        wahoo_workout_token: document.wahoo_workout_token,
        last_error: document.last_error,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(PlannedWorkoutWahooSyncError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(PlannedWorkoutWahooSyncError::Repository)?,
        last_synced_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.last_synced_at,
            document.last_synced_at_epoch_seconds,
        ),
    })
}

fn map_status(value: &str) -> Result<PlannedWorkoutWahooSyncStatus, PlannedWorkoutWahooSyncError> {
    match value {
        "unsynced" => Ok(PlannedWorkoutWahooSyncStatus::Unsynced),
        "pending" => Ok(PlannedWorkoutWahooSyncStatus::Pending),
        "synced" => Ok(PlannedWorkoutWahooSyncStatus::Synced),
        "modified" => Ok(PlannedWorkoutWahooSyncStatus::Modified),
        "failed" => Ok(PlannedWorkoutWahooSyncStatus::Failed),
        other => Err(PlannedWorkoutWahooSyncError::Repository(format!(
            "unknown planned workout Wahoo sync status: {other}"
        ))),
    }
}
