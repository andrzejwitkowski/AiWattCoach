use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::training_plan::{
    BoxFuture, TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
    TrainingPlanSnapshot,
};

use super::{
    training_plan_shared::{
        map_document_to_planned_workout, map_planned_workout_to_document, PlannedWorkoutDocument,
    },
    training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
};

#[derive(Clone)]
pub struct MongoTrainingPlanProjectionRepository {
    collection: Collection<TrainingPlanProjectedDayDocument>,
    snapshot_repository: MongoTrainingPlanSnapshotRepository,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanProjectedDayDocument {
    user_id: String,
    workout_id: String,
    operation_key: String,
    date: String,
    rest_day: bool,
    workout: Option<PlannedWorkoutDocument>,
    active: bool,
    superseded_at_epoch_seconds: Option<i64>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoTrainingPlanProjectionRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_plan_projected_days"),
            snapshot_repository: MongoTrainingPlanSnapshotRepository::new(client, database),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TrainingPlanError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "operation_key": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "training_plan_projected_days_user_operation_date_unique"
                                    .to_string(),
                            )
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "active": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_plan_projected_days_user_active_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "operation_key": 1, "active": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_plan_projected_days_operation_active_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

        self.snapshot_repository
            .collection()
            .create_indexes([IndexModel::builder()
                .keys(doc! { "operation_key": 1 })
                .options(
                    IndexOptions::builder()
                        .name("training_plan_snapshots_operation_key_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl TrainingPlanProjectionRepository for MongoTrainingPlanProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! { "user_id": &user_id, "active": true })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .collect()
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! { "operation_key": &operation_key, "active": true })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .collect()
        })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>>
    {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_repository.collection();
        let snapshot_document =
            MongoTrainingPlanSnapshotRepository::map_snapshot_to_document(&snapshot);
        let projected_day_documents = projected_days
            .iter()
            .map(map_projected_day_to_document)
            .collect::<Vec<_>>();
        let today = today.to_string();
        let snapshot_clone = snapshot.clone();
        Box::pin(async move {
            collection
                .update_many(
                    doc! {
                        "user_id": &snapshot.user_id,
                        "active": true,
                        "date": {
                            "$gte": std::cmp::max(today.as_str(), snapshot.start_date.as_str()),
                            "$lte": &snapshot.end_date,
                        },
                    },
                    doc! {
                        "$set": {
                            "active": false,
                            "superseded_at_epoch_seconds": replaced_at_epoch_seconds,
                            "updated_at_epoch_seconds": replaced_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            snapshot_collection
                .replace_one(
                    doc! { "operation_key": &snapshot.operation_key },
                    &snapshot_document,
                )
                .upsert(true)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            if !projected_day_documents.is_empty() {
                collection
                    .insert_many(projected_day_documents)
                    .await
                    .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            }

            Ok((
                snapshot_clone,
                projected_days
                    .into_iter()
                    .filter(|day| day.active)
                    .collect(),
            ))
        })
    }
}

fn map_projected_day_to_document(
    day: &TrainingPlanProjectedDay,
) -> TrainingPlanProjectedDayDocument {
    TrainingPlanProjectedDayDocument {
        user_id: day.user_id.clone(),
        workout_id: day.workout_id.clone(),
        operation_key: day.operation_key.clone(),
        date: day.date.clone(),
        rest_day: day.rest_day,
        workout: day.workout.as_ref().map(map_planned_workout_to_document),
        active: day.active,
        superseded_at_epoch_seconds: day.superseded_at_epoch_seconds,
        created_at_epoch_seconds: day.created_at_epoch_seconds,
        updated_at_epoch_seconds: day.updated_at_epoch_seconds,
    }
}

fn map_document_to_projected_day(
    document: TrainingPlanProjectedDayDocument,
) -> Result<TrainingPlanProjectedDay, TrainingPlanError> {
    Ok(TrainingPlanProjectedDay {
        user_id: document.user_id,
        workout_id: document.workout_id,
        operation_key: document.operation_key,
        date: document.date,
        rest_day: document.rest_day,
        workout: document
            .workout
            .map(map_document_to_planned_workout)
            .transpose()?,
        active: document.active,
        superseded_at_epoch_seconds: document.superseded_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
