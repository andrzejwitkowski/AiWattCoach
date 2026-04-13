use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson},
    Collection,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::domain::planned_workouts::{
    BoxFuture as PlannedWorkoutBoxFuture, PlannedWorkout, PlannedWorkoutContent,
    PlannedWorkoutError, PlannedWorkoutLine, PlannedWorkoutRepeat, PlannedWorkoutRepository,
    PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
};

use super::training_plan_shared::{map_document_to_planned_workout, PlannedWorkoutDocument};

#[derive(Clone)]
pub struct MongoPlannedWorkoutRepository {
    collection: Collection<TrainingPlanProjectedDayDocument>,
    snapshot_collection: Collection<super::training_plan_snapshots::TrainingPlanSnapshotDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct TrainingPlanProjectedDayDocument {
    user_id: String,
    operation_key: String,
    date: String,
    workout: Option<PlannedWorkoutDocument>,
}

impl MongoPlannedWorkoutRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_plan_projected_days"),
            snapshot_collection: client
                .database(database.as_ref())
                .collection("training_plan_snapshots"),
        }
    }
}

impl PlannedWorkoutRepository for MongoPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "superseded_at_epoch_seconds": Bson::Null,
                    "workout": { "$ne": Bson::Null },
                })
                .sort(doc! { "date": 1, "operation_key": 1 })
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                .into_iter()
                .collect::<Vec<_>>();

            let snapshot_start_dates =
                load_snapshot_start_dates(&snapshot_collection, Some(&user_id)).await?;

            documents
                .into_iter()
                .filter(|document| {
                    snapshot_start_dates
                        .get(&document.operation_key)
                        .map(|start_date| document.date > *start_date)
                        .unwrap_or(false)
                })
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let collection = self.collection.clone();
        let snapshot_collection = self.snapshot_collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &oldest,
                        "$lte": &newest,
                    },
                    "superseded_at_epoch_seconds": Bson::Null,
                    "workout": { "$ne": Bson::Null },
                })
                .sort(doc! { "date": 1, "operation_key": 1 })
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                .into_iter()
                .collect::<Vec<_>>();

            let snapshot_start_dates =
                load_snapshot_start_dates(&snapshot_collection, Some(&user_id)).await?;

            documents
                .into_iter()
                .filter(|document| {
                    snapshot_start_dates
                        .get(&document.operation_key)
                        .map(|start_date| document.date > *start_date)
                        .unwrap_or(false)
                })
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn upsert(
        &self,
        _workout: PlannedWorkout,
    ) -> PlannedWorkoutBoxFuture<Result<PlannedWorkout, PlannedWorkoutError>> {
        Box::pin(async {
            Err(PlannedWorkoutError::Repository(
                "planned workout bridge repository is read-only; persist through training plan projections"
                    .to_string(),
            ))
        })
    }
}

async fn load_snapshot_start_dates(
    snapshot_collection: &Collection<super::training_plan_snapshots::TrainingPlanSnapshotDocument>,
    user_id: Option<&str>,
) -> Result<HashMap<String, String>, PlannedWorkoutError> {
    let filter = user_id
        .map(|user_id| doc! { "user_id": user_id })
        .unwrap_or_else(|| doc! {});

    snapshot_collection
        .find(filter)
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .into_iter()
        .map(|snapshot| {
            super::training_plan_snapshots::MongoTrainingPlanSnapshotRepository::map_document_to_snapshot(snapshot)
                .map(|snapshot| (snapshot.operation_key, snapshot.start_date))
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))
        })
        .collect()
}

fn map_document_to_domain(
    document: TrainingPlanProjectedDayDocument,
) -> Result<PlannedWorkout, PlannedWorkoutError> {
    let workout = document.workout.ok_or_else(|| {
        PlannedWorkoutError::Repository(
            "projected day is missing planned workout payload".to_string(),
        )
    })?;

    Ok(PlannedWorkout::new(
        format!("{}:{}", document.operation_key, document.date),
        document.user_id,
        document.date,
        PlannedWorkoutContent {
            lines: map_workout_lines(
                map_document_to_planned_workout(workout)
                    .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                    .lines,
            ),
        },
    ))
}

fn map_workout_lines(
    lines: Vec<crate::domain::intervals::PlannedWorkoutLine>,
) -> Vec<PlannedWorkoutLine> {
    lines.into_iter().map(map_workout_line).collect()
}

fn map_workout_line(line: crate::domain::intervals::PlannedWorkoutLine) -> PlannedWorkoutLine {
    match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => {
            PlannedWorkoutLine::Text(PlannedWorkoutText { text: text.text })
        }
        crate::domain::intervals::PlannedWorkoutLine::Repeat(repeat) => {
            PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat {
                title: repeat.title,
                count: repeat.count,
            })
        }
        crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                duration_seconds: step.duration_seconds,
                kind: match step.kind {
                    crate::domain::intervals::PlannedWorkoutStepKind::Steady => {
                        PlannedWorkoutStepKind::Steady
                    }
                    crate::domain::intervals::PlannedWorkoutStepKind::Ramp => {
                        PlannedWorkoutStepKind::Ramp
                    }
                },
                target: match step.target {
                    crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max } => {
                        PlannedWorkoutTarget::PercentFtp { min, max }
                    }
                    crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max } => {
                        PlannedWorkoutTarget::WattsRange { min, max }
                    }
                },
            })
        }
    }
}
