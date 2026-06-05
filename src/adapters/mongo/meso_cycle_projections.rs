use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use super::training_plan_shared::{
    map_document_to_planned_workout, map_planned_workout_to_document, PlannedWorkoutDocument,
};
use crate::domain::meso_cycle::{
    BoxFuture, MesoCycleError, MesoCycleProjectedDay, MesoCycleProjectionRepository,
};

#[derive(Clone)]
pub struct MongoMesoCycleProjectionRepository {
    collection: Collection<MesoCycleProjectedDayDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MesoCycleProjectedDayDocument {
    user_id: String,
    operation_key: String,
    date: String,
    #[serde(default)]
    rest_day: bool,
    #[serde(default)]
    rest_day_reason: Option<String>,
    workout: Option<PlannedWorkoutDocument>,
    superseded_at_epoch_seconds: Option<i64>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoMesoCycleProjectionRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("meso_cycle_projected_days"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), MesoCycleError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "operation_key": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "meso_cycle_projected_days_user_operation_date_unique".to_string(),
                            )
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "superseded_at_epoch_seconds": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("meso_cycle_projected_days_user_unsuperseded_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl MesoCycleProjectionRepository for MongoMesoCycleProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<MesoCycleProjectedDay>, MesoCycleError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .collect()
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<MesoCycleProjectedDay>, MesoCycleError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "operation_key": &operation_key,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .collect()
        })
    }

    fn replace_window(
        &self,
        user_id: &str,
        operation_key: &str,
        projected_days: Vec<MesoCycleProjectedDay>,
        replaced_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), MesoCycleError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            if projected_days.is_empty() {
                return Err(MesoCycleError::Validation(
                    "meso cycle projection window must contain at least one projected day"
                        .to_string(),
                ));
            }

            let documents = projected_days
                .iter()
                .map(map_projected_day_to_document)
                .collect::<Result<Vec<_>, _>>()?;

            for document in &documents {
                collection
                    .replace_one(
                        doc! {
                            "user_id": &document.user_id,
                            "operation_key": &document.operation_key,
                            "date": &document.date,
                        },
                        document,
                    )
                    .upsert(true)
                    .await
                    .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            }

            let active_dates: Vec<&str> = documents
                .iter()
                .map(|document| document.date.as_str())
                .collect();
            collection
                .update_many(
                    doc! {
                        "user_id": &user_id,
                        "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                        "$or": [
                            { "operation_key": { "$ne": &operation_key } },
                            { "date": { "$nin": &active_dates } },
                        ],
                    },
                    doc! {
                        "$set": {
                            "superseded_at_epoch_seconds": replaced_at_epoch_seconds,
                            "updated_at_epoch_seconds": replaced_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_projected_day_to_document(
    day: &MesoCycleProjectedDay,
) -> Result<MesoCycleProjectedDayDocument, MesoCycleError> {
    Ok(MesoCycleProjectedDayDocument {
        user_id: day.user_id.clone(),
        operation_key: day.operation_key.clone(),
        date: day.date.clone(),
        rest_day: day.rest_day,
        rest_day_reason: day.rest_day_reason.clone(),
        workout: day
            .workout
            .as_ref()
            .map(map_planned_workout_to_document)
            .transpose()
            .map_err(|error| MesoCycleError::Repository(error.to_string()))?,
        superseded_at_epoch_seconds: day.superseded_at_epoch_seconds,
        created_at_epoch_seconds: day.created_at_epoch_seconds,
        updated_at_epoch_seconds: day.updated_at_epoch_seconds,
    })
}

fn map_document_to_projected_day(
    document: MesoCycleProjectedDayDocument,
) -> Result<MesoCycleProjectedDay, MesoCycleError> {
    Ok(MesoCycleProjectedDay {
        user_id: document.user_id,
        operation_key: document.operation_key,
        date: document.date,
        rest_day: document.rest_day,
        rest_day_reason: document.rest_day_reason,
        workout: document
            .workout
            .map(map_document_to_planned_workout)
            .transpose()
            .map_err(|error| MesoCycleError::Repository(error.to_string()))?,
        superseded_at_epoch_seconds: document.superseded_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
