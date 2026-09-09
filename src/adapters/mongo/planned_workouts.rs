use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::planned_workouts::{
    BoxFuture as PlannedWorkoutBoxFuture, PlannedWorkout, PlannedWorkoutContent,
    PlannedWorkoutError, PlannedWorkoutRepository,
};

use super::imported_planned_workouts::{
    delete_imported_planned_workouts_for_user_date_keeping, map_imported_document_to_domain,
    map_imported_workout_to_document, ImportedPlannedWorkoutDocument,
};
use super::training_plan_shared::{
    map_document_to_planned_workout, map_intervals_lines_to_canonical, PlannedWorkoutDocument,
};

#[derive(Clone)]
pub struct MongoPlannedWorkoutRepository {
    projected_collection: Collection<TrainingPlanProjectedDayDocument>,
    snapshot_collection: Collection<TrainingPlanSnapshotLookupDocument>,
    imported_collection: Collection<ImportedPlannedWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct TrainingPlanProjectedDayDocument {
    user_id: String,
    operation_key: String,
    date: String,
    #[serde(default)]
    rest_day: bool,
    #[serde(default)]
    rest_day_reason: Option<String>,
    workout: Option<PlannedWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanSnapshotLookupDocument {
    operation_key: String,
    start_date: String,
}

impl MongoPlannedWorkoutRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            projected_collection: client
                .database(database.as_ref())
                .collection("training_plan_projected_days"),
            snapshot_collection: client
                .database(database.as_ref())
                .collection("training_plan_snapshots"),
            imported_collection: client
                .database(database.as_ref())
                .collection("planned_workouts"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), PlannedWorkoutError> {
        self.imported_collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "planned_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workouts_user_planned_workout_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workouts_user_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl PlannedWorkoutRepository for MongoPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let projected_collection = self.projected_collection.clone();
        let snapshot_collection = self.snapshot_collection.clone();
        let imported_collection = self.imported_collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = load_projected_workouts(
                &projected_collection,
                &snapshot_collection,
                &user_id,
                None,
            )
            .await?;
            workouts.extend(load_imported_workouts(&imported_collection, &user_id, None).await?);
            dedupe_prefer_imported_workouts(&mut workouts);
            sort_planned_workouts(&mut workouts);
            Ok(workouts)
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> PlannedWorkoutBoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let projected_collection = self.projected_collection.clone();
        let snapshot_collection = self.snapshot_collection.clone();
        let imported_collection = self.imported_collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let mut workouts = load_projected_workouts(
                &projected_collection,
                &snapshot_collection,
                &user_id,
                Some((&oldest, &newest)),
            )
            .await?;
            workouts.extend(
                load_imported_workouts(&imported_collection, &user_id, Some((&oldest, &newest)))
                    .await?,
            );
            dedupe_prefer_imported_workouts(&mut workouts);
            sort_planned_workouts(&mut workouts);
            Ok(workouts)
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> PlannedWorkoutBoxFuture<Result<PlannedWorkout, PlannedWorkoutError>> {
        let imported_collection = self.imported_collection.clone();
        let document = map_imported_workout_to_document(&workout);
        Box::pin(async move {
            imported_collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "planned_workout_id": &document.planned_workout_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
            Ok(workout)
        })
    }

    fn delete_imported_for_user_date_keeping(
        &self,
        user_id: &str,
        date: &str,
        keep_planned_workout_ids: Vec<String>,
    ) -> PlannedWorkoutBoxFuture<Result<u64, PlannedWorkoutError>> {
        let imported_collection = self.imported_collection.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move {
            delete_imported_planned_workouts_for_user_date_keeping(
                &imported_collection,
                &user_id,
                &date,
                keep_planned_workout_ids,
            )
            .await
        })
    }

    fn delete_by_user_id_and_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> PlannedWorkoutBoxFuture<Result<(), PlannedWorkoutError>> {
        let imported_collection = self.imported_collection.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            imported_collection
                .delete_one(doc! {
                    "user_id": &user_id,
                    "planned_workout_id": &planned_workout_id,
                })
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
            Ok(())
        })
    }
}

async fn load_projected_workouts(
    projected_collection: &Collection<TrainingPlanProjectedDayDocument>,
    snapshot_collection: &Collection<TrainingPlanSnapshotLookupDocument>,
    user_id: &str,
    range: Option<(&str, &str)>,
) -> Result<Vec<PlannedWorkout>, PlannedWorkoutError> {
    let mut filter = doc! {
        "user_id": user_id,
        "superseded_at_epoch_seconds": Bson::Null,
        "$or": [
            { "workout": { "$ne": Bson::Null } },
            { "rest_day": true },
        ],
    };
    if let Some((oldest, newest)) = range {
        filter.insert(
            "date",
            doc! {
                "$gte": oldest,
                "$lte": newest,
            },
        );
    }

    let documents = projected_collection
        .find(filter)
        .sort(doc! { "date": 1, "operation_key": 1 })
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;

    let operation_keys = documents
        .iter()
        .map(|document| document.operation_key.as_str())
        .collect::<Vec<_>>();
    let snapshot_start_dates =
        load_snapshot_start_dates(snapshot_collection, user_id, &operation_keys).await?;

    documents
        .into_iter()
        .filter(|document| {
            snapshot_start_dates
                .get(&document.operation_key)
                .map(|start_date| document.date >= *start_date)
                .unwrap_or(false)
        })
        .map(map_projected_document_to_domain)
        .collect()
}

async fn load_imported_workouts(
    imported_collection: &Collection<ImportedPlannedWorkoutDocument>,
    user_id: &str,
    range: Option<(&str, &str)>,
) -> Result<Vec<PlannedWorkout>, PlannedWorkoutError> {
    let mut filter = doc! { "user_id": user_id };
    if let Some((oldest, newest)) = range {
        filter.insert(
            "date",
            doc! {
                "$gte": oldest,
                "$lte": newest,
            },
        );
    }

    imported_collection
        .find(filter)
        .sort(doc! { "date": 1, "planned_workout_id": 1 })
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .into_iter()
        .map(map_imported_document_to_domain)
        .collect()
}

async fn load_snapshot_start_dates(
    snapshot_collection: &Collection<TrainingPlanSnapshotLookupDocument>,
    user_id: &str,
    operation_keys: &[&str],
) -> Result<HashMap<String, String>, PlannedWorkoutError> {
    if operation_keys.is_empty() {
        return Ok(HashMap::new());
    }

    let filter = doc! { "user_id": user_id, "operation_key": { "$in": operation_keys } };

    snapshot_collection
        .find(filter)
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
        .into_iter()
        .map(|snapshot| Ok((snapshot.operation_key, snapshot.start_date)))
        .collect()
}

fn map_projected_document_to_domain(
    document: TrainingPlanProjectedDayDocument,
) -> Result<PlannedWorkout, PlannedWorkoutError> {
    if document.rest_day {
        return Ok(PlannedWorkout::new(
            format!("{}:{}", document.operation_key, document.date),
            document.user_id,
            document.date,
            PlannedWorkoutContent { lines: Vec::new() },
        )
        .with_event_metadata(
            Some("Rest Day".to_string()),
            document.rest_day_reason.clone(),
            Some("Ride".to_string()),
        )
        .as_rest_day(document.rest_day_reason));
    }

    let workout = document.workout.ok_or_else(|| {
        PlannedWorkoutError::Repository(
            "projected day is missing planned workout payload".to_string(),
        )
    })?;

    let planned_workout = PlannedWorkout::new(
        format!("{}:{}", document.operation_key, document.date),
        document.user_id,
        document.date,
        PlannedWorkoutContent {
            lines: map_intervals_lines_to_canonical(
                map_document_to_planned_workout(workout)
                    .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
                    .lines,
            ),
        },
    )
    .with_event_metadata(None, None, Some("Ride".to_string()));

    Ok(planned_workout)
}

fn sort_planned_workouts(workouts: &mut [PlannedWorkout]) {
    workouts.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.planned_workout_id.cmp(&right.planned_workout_id))
    });
}

fn dedupe_prefer_imported_workouts(workouts: &mut Vec<PlannedWorkout>) {
    // Projected workouts are always loaded before imported workouts (see callers).
    // When the same planned_workout_id appears in both collections, the imported
    // (locally stored) version is a user or AI-coach override and must win,
    // regardless of whether it carries a name or description. Simply inserting in
    // order means the last writer (imported) always replaces the first (projected).
    let mut deduped = HashMap::<String, PlannedWorkout>::new();
    for workout in workouts.drain(..) {
        deduped.insert(workout.planned_workout_id.clone(), workout);
    }
    *workouts = deduped.into_values().collect();
}

#[cfg(test)]
mod tests {
    use crate::domain::planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutText,
    };

    use super::{dedupe_prefer_imported_workouts, sort_planned_workouts};

    #[test]
    fn sort_planned_workouts_orders_mixed_sources_by_date_then_id() {
        let mut workouts = vec![
            PlannedWorkout::new(
                "imported-2".to_string(),
                "user-1".to_string(),
                "2026-05-11".to_string(),
                PlannedWorkoutContent { lines: Vec::new() },
            ),
            PlannedWorkout::new(
                "projected-1".to_string(),
                "user-1".to_string(),
                "2026-05-10".to_string(),
                PlannedWorkoutContent { lines: Vec::new() },
            ),
            PlannedWorkout::new(
                "imported-1".to_string(),
                "user-1".to_string(),
                "2026-05-10".to_string(),
                PlannedWorkoutContent { lines: Vec::new() },
            ),
        ];

        sort_planned_workouts(&mut workouts);

        assert_eq!(workouts[0].planned_workout_id, "imported-1");
        assert_eq!(workouts[1].planned_workout_id, "projected-1");
        assert_eq!(workouts[2].planned_workout_id, "imported-2");
    }

    #[test]
    fn dedupe_prefer_imported_workouts_keeps_local_override_over_projected_copy() {
        let projected = PlannedWorkout::new(
            "training-plan:user-1:w1:2026-05-10".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            PlannedWorkoutContent {
                lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Projected".to_string(),
                })],
            },
        );
        let imported = PlannedWorkout::new(
            "training-plan:user-1:w1:2026-05-10".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            PlannedWorkoutContent {
                lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Local Override".to_string(),
                })],
            },
        )
        .with_event_metadata(
            Some("Local Override".to_string()),
            Some("edited".to_string()),
            None,
        );

        let mut workouts = vec![projected, imported.clone()];

        dedupe_prefer_imported_workouts(&mut workouts);

        assert_eq!(workouts, vec![imported]);
    }
}
