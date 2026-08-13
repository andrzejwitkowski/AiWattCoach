use std::collections::{BTreeSet, HashMap};

use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::training_plan::{
    BoxFuture, TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
    TrainingPlanReplacementResult, TrainingPlanSnapshot,
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
    #[serde(default)]
    rest_day: bool,
    #[serde(default)]
    rest_day_reason: Option<String>,
    workout: Option<PlannedWorkoutDocument>,
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
                    .keys(doc! { "user_id": 1, "superseded_at_epoch_seconds": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_plan_projected_days_user_unsuperseded_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "operation_key": 1, "superseded_at_epoch_seconds": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "training_plan_projected_days_operation_unsuperseded_date"
                                    .to_string(),
                            )
                            .build(),
                    )
                    .build(),
            ])
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
        let snapshot_collection = self.snapshot_repository.collection();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            let snapshot_documents = snapshot_collection
                .find(doc! { "user_id": &user_id })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let snapshot_start_dates = snapshot_documents
                .into_iter()
                .map(MongoTrainingPlanSnapshotRepository::map_document_to_snapshot)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|snapshot| (snapshot.operation_key, snapshot.start_date))
                .collect::<HashMap<_, _>>();

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .filter_map(|day| match day {
                    Ok(day) => snapshot_start_dates
                        .get(&day.operation_key)
                        .filter(|start_date| day.date >= **start_date)
                        .map(|_| Ok(day)),
                    Err(error) => Some(Err(error)),
                })
                .collect()
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_repository.collection();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "operation_key": &operation_key,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            let snapshot = snapshot_collection
                .find_one(doc! { "operation_key": &operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let Some(snapshot) = snapshot else {
                return Ok(Vec::new());
            };
            let snapshot = MongoTrainingPlanSnapshotRepository::map_document_to_snapshot(snapshot)?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .filter_map(|day| match day {
                    Ok(day) if day.date >= snapshot.start_date => Some(Ok(day)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect()
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_repository.collection();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "operation_key": &operation_key,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": 1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            let snapshot = snapshot_collection
                .find_one(doc! { "user_id": &user_id, "operation_key": &operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            let Some(snapshot) = snapshot else {
                return Ok(Vec::new());
            };
            let snapshot = MongoTrainingPlanSnapshotRepository::map_document_to_snapshot(snapshot)?;

            documents
                .into_iter()
                .map(map_document_to_projected_day)
                .filter_map(|day| match day {
                    Ok(day) if day.date >= snapshot.start_date => Some(Ok(day)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect()
        })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>> {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_repository.collection();
        let today = today.to_string();
        Box::pin(async move {
            let snapshot_document =
                MongoTrainingPlanSnapshotRepository::map_snapshot_to_document(&snapshot)?;
            let projected_day_documents = projected_days
                .iter()
                .map(map_projected_day_to_document)
                .collect::<Result<Vec<_>, _>>()?;
            validate_replacement_scope(&snapshot, &projected_days)?;
            if projected_day_documents.is_empty() {
                return Err(TrainingPlanError::Validation(
                    "training plan projection window must contain at least one projected day"
                        .to_string(),
                ));
            }

            let superseded_range_start =
                std::cmp::max(today.as_str(), snapshot.start_date.as_str());
            let same_operation_active_date_range = load_active_operation_date_range(
                &collection,
                &snapshot.user_id,
                &snapshot.operation_key,
            )
            .await?;

            let max_active_date: Option<String> = collection
                .clone()
                .clone_with_type::<mongodb::bson::Document>()
                .find(doc! {
                    "user_id": &snapshot.user_id,
                    "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                })
                .sort(doc! { "date": -1 })
                .limit(1)
                .projection(doc! { "date": 1, "_id": 0 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .try_next()
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .and_then(|doc| doc.get_str("date").ok().map(String::from));

            let superseded_range_end = max_active_date
                .as_ref()
                .map(|date| std::cmp::max(snapshot.end_date.as_str(), date.as_str()))
                .unwrap_or(snapshot.end_date.as_str())
                .to_string();
            let superseded_refresh_start = same_operation_active_date_range
                .as_ref()
                .map(|(start, _)| std::cmp::min(superseded_range_start, start.as_str()))
                .unwrap_or(superseded_range_start)
                .to_string();
            let superseded_refresh_end = same_operation_active_date_range
                .as_ref()
                .map(|(_, end)| std::cmp::max(superseded_range_end.as_str(), end.as_str()))
                .unwrap_or(superseded_range_end.as_str())
                .to_string();

            let update_result = collection
                .update_many(
                    doc! {
                        "user_id": &snapshot.user_id,
                        "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                        "$or": [
                            {
                                "date": {
                                    "$gte": superseded_range_start,
                                    "$lte": &superseded_range_end,
                                },
                            },
                            {
                                "operation_key": &snapshot.operation_key,
                            },
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
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            let superseded_date_range = if update_result.matched_count == 0 {
                None
            } else {
                Some((superseded_refresh_start, superseded_refresh_end))
            };

            snapshot_collection
                .replace_one(
                    doc! { "operation_key": &snapshot.operation_key },
                    &snapshot_document,
                )
                .upsert(true)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            for projected_day_document in projected_day_documents {
                collection
                    .replace_one(
                        doc! {
                            "user_id": &projected_day_document.user_id,
                            "operation_key": &projected_day_document.operation_key,
                            "date": &projected_day_document.date,
                        },
                        &projected_day_document,
                    )
                    .upsert(true)
                    .await
                    .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            }

            Ok(TrainingPlanReplacementResult {
                snapshot,
                projected_days,
                superseded_date_range,
            })
        })
    }

    fn supersede_active_dates(
        &self,
        user_id: &str,
        dates: &[String],
        superseded_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<(String, String)>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let dates = dates.to_vec();
        Box::pin(async move {
            if dates.is_empty() {
                return Ok(None);
            }

            let update_result = collection
                .update_many(
                    doc! {
                        "user_id": &user_id,
                        "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
                        "date": { "$in": &dates },
                    },
                    doc! {
                        "$set": {
                            "superseded_at_epoch_seconds": superseded_at_epoch_seconds,
                            "updated_at_epoch_seconds": superseded_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            if update_result.matched_count == 0 {
                return Ok(None);
            }

            let mut sorted = dates;
            sorted.sort();
            Ok(Some((
                sorted.first().cloned().unwrap_or_default(),
                sorted.last().cloned().unwrap_or_default(),
            )))
        })
    }
}

async fn load_active_operation_date_range(
    collection: &Collection<TrainingPlanProjectedDayDocument>,
    user_id: &str,
    operation_key: &str,
) -> Result<Option<(String, String)>, TrainingPlanError> {
    let lookup_collection = collection
        .clone()
        .clone_with_type::<mongodb::bson::Document>();
    let filter = doc! {
        "user_id": user_id,
        "operation_key": operation_key,
        "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
    };

    let first_date = lookup_collection
        .find_one(filter.clone())
        .sort(doc! { "date": 1 })
        .projection(doc! { "date": 1, "_id": 0 })
        .await
        .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
        .and_then(|doc| doc.get_str("date").ok().map(String::from));
    let last_date = collection
        .clone()
        .clone_with_type::<mongodb::bson::Document>()
        .find_one(filter)
        .sort(doc! { "date": -1 })
        .projection(doc! { "date": 1, "_id": 0 })
        .await
        .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
        .and_then(|doc| doc.get_str("date").ok().map(String::from));

    Ok(first_date.zip(last_date))
}

fn map_projected_day_to_document(
    day: &TrainingPlanProjectedDay,
) -> Result<TrainingPlanProjectedDayDocument, TrainingPlanError> {
    Ok(TrainingPlanProjectedDayDocument {
        user_id: day.user_id.clone(),
        workout_id: day.workout_id.clone(),
        operation_key: day.operation_key.clone(),
        date: day.date.clone(),
        rest_day: day.rest_day,
        rest_day_reason: day.rest_day_reason.clone(),
        workout: day
            .workout
            .as_ref()
            .map(map_planned_workout_to_document)
            .transpose()?,
        superseded_at_epoch_seconds: day.superseded_at_epoch_seconds,
        created_at_epoch_seconds: day.created_at_epoch_seconds,
        updated_at_epoch_seconds: day.updated_at_epoch_seconds,
    })
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
        rest_day_reason: document.rest_day_reason,
        workout: document
            .workout
            .map(map_document_to_planned_workout)
            .transpose()?,
        superseded_at_epoch_seconds: document.superseded_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}

fn validate_replacement_scope(
    snapshot: &TrainingPlanSnapshot,
    projected_days: &[TrainingPlanProjectedDay],
) -> Result<(), TrainingPlanError> {
    let expected_dates = snapshot
        .days
        .iter()
        .map(|day| day.date.as_str())
        .collect::<BTreeSet<_>>();

    for day in projected_days {
        if day.user_id != snapshot.user_id
            || day.workout_id != snapshot.workout_id
            || day.operation_key != snapshot.operation_key
            || !expected_dates.contains(day.date.as_str())
        {
            return Err(TrainingPlanError::Validation(
                "projected day replacement set does not match snapshot scope".to_string(),
            ));
        }
    }

    Ok(())
}
