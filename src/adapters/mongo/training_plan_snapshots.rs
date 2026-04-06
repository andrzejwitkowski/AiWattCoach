use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};

use crate::domain::training_plan::{
    BoxFuture, TrainingPlanDay, TrainingPlanError, TrainingPlanSnapshot,
    TrainingPlanSnapshotRepository,
};

use super::training_plan_shared::{
    map_document_to_planned_workout, map_planned_workout_to_document, PlannedWorkoutDocument,
};

#[derive(Clone)]
pub struct MongoTrainingPlanSnapshotRepository {
    collection: Collection<TrainingPlanSnapshotDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TrainingPlanSnapshotDocument {
    user_id: String,
    workout_id: String,
    operation_key: String,
    saved_at_epoch_seconds: i64,
    start_date: String,
    end_date: String,
    days: Vec<TrainingPlanDayDocument>,
    created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanDayDocument {
    date: String,
    rest_day: bool,
    workout: Option<PlannedWorkoutDocument>,
}

impl MongoTrainingPlanSnapshotRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_plan_snapshots"),
        }
    }

    pub(super) fn collection(&self) -> Collection<TrainingPlanSnapshotDocument> {
        self.collection.clone()
    }

    pub(super) fn map_snapshot_to_document(
        snapshot: &TrainingPlanSnapshot,
    ) -> TrainingPlanSnapshotDocument {
        TrainingPlanSnapshotDocument {
            user_id: snapshot.user_id.clone(),
            workout_id: snapshot.workout_id.clone(),
            operation_key: snapshot.operation_key.clone(),
            saved_at_epoch_seconds: snapshot.saved_at_epoch_seconds,
            start_date: snapshot.start_date.clone(),
            end_date: snapshot.end_date.clone(),
            days: snapshot.days.iter().map(map_day_to_document).collect(),
            created_at_epoch_seconds: snapshot.created_at_epoch_seconds,
        }
    }

    pub(super) fn map_document_to_snapshot(
        document: TrainingPlanSnapshotDocument,
    ) -> Result<TrainingPlanSnapshot, TrainingPlanError> {
        Ok(TrainingPlanSnapshot {
            user_id: document.user_id,
            workout_id: document.workout_id,
            operation_key: document.operation_key,
            saved_at_epoch_seconds: document.saved_at_epoch_seconds,
            start_date: document.start_date,
            end_date: document.end_date,
            days: document
                .days
                .into_iter()
                .map(map_document_to_day)
                .collect::<Result<Vec<_>, _>>()?,
            created_at_epoch_seconds: document.created_at_epoch_seconds,
        })
    }
}

impl TrainingPlanSnapshotRepository for MongoTrainingPlanSnapshotRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSnapshot>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "operation_key": &operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            document.map(Self::map_document_to_snapshot).transpose()
        })
    }
}

fn map_day_to_document(day: &TrainingPlanDay) -> TrainingPlanDayDocument {
    TrainingPlanDayDocument {
        date: day.date.clone(),
        rest_day: day.rest_day,
        workout: day.workout.as_ref().map(map_planned_workout_to_document),
    }
}

fn map_document_to_day(
    document: TrainingPlanDayDocument,
) -> Result<TrainingPlanDay, TrainingPlanError> {
    Ok(TrainingPlanDay {
        date: document.date,
        rest_day: document.rest_day,
        workout: document
            .workout
            .map(map_document_to_planned_workout)
            .transpose()?,
    })
}
