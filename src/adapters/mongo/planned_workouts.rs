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
    PlannedWorkoutError, PlannedWorkoutLine, PlannedWorkoutRepeat, PlannedWorkoutRepository,
    PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
};

use super::training_plan_shared::{map_document_to_planned_workout, PlannedWorkoutDocument};

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
    workout: Option<PlannedWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanSnapshotLookupDocument {
    operation_key: String,
    start_date: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ImportedPlannedWorkoutDocument {
    user_id: String,
    planned_workout_id: String,
    date: String,
    workout: StoredPlannedWorkoutContentDocument,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredPlannedWorkoutContentDocument {
    lines: Vec<StoredPlannedWorkoutLineDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredPlannedWorkoutLineDocument {
    Text {
        text: String,
    },
    Repeat {
        title: Option<String>,
        count: i64,
    },
    Step {
        duration_seconds: i32,
        step_kind: String,
        target: StoredPlannedWorkoutTargetDocument,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredPlannedWorkoutTargetDocument {
    PercentFtp { min: f64, max: f64 },
    WattsRange { min: i32, max: i32 },
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
        "workout": { "$ne": Bson::Null },
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
                .map(|start_date| document.date > *start_date)
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

fn map_imported_workout_to_document(workout: &PlannedWorkout) -> ImportedPlannedWorkoutDocument {
    ImportedPlannedWorkoutDocument {
        user_id: workout.user_id.clone(),
        planned_workout_id: workout.planned_workout_id.clone(),
        date: workout.date.clone(),
        workout: StoredPlannedWorkoutContentDocument {
            lines: workout
                .workout
                .lines
                .iter()
                .map(map_domain_line_to_stored_line)
                .collect(),
        },
    }
}

fn map_imported_document_to_domain(
    document: ImportedPlannedWorkoutDocument,
) -> Result<PlannedWorkout, PlannedWorkoutError> {
    Ok(PlannedWorkout::new(
        document.planned_workout_id,
        document.user_id,
        document.date,
        PlannedWorkoutContent {
            lines: document
                .workout
                .lines
                .into_iter()
                .map(map_stored_line_to_domain)
                .collect::<Result<Vec<_>, _>>()?,
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

fn map_domain_line_to_stored_line(line: &PlannedWorkoutLine) -> StoredPlannedWorkoutLineDocument {
    match line {
        PlannedWorkoutLine::Text(text) => StoredPlannedWorkoutLineDocument::Text {
            text: text.text.clone(),
        },
        PlannedWorkoutLine::Repeat(repeat) => StoredPlannedWorkoutLineDocument::Repeat {
            title: repeat.title.clone(),
            count: repeat.count as i64,
        },
        PlannedWorkoutLine::Step(step) => StoredPlannedWorkoutLineDocument::Step {
            duration_seconds: step.duration_seconds,
            step_kind: map_step_kind_to_str(&step.kind).to_string(),
            target: map_domain_target_to_stored_target(&step.target),
        },
    }
}

fn map_stored_line_to_domain(
    line: StoredPlannedWorkoutLineDocument,
) -> Result<PlannedWorkoutLine, PlannedWorkoutError> {
    match line {
        StoredPlannedWorkoutLineDocument::Text { text } => {
            Ok(PlannedWorkoutLine::Text(PlannedWorkoutText { text }))
        }
        StoredPlannedWorkoutLineDocument::Repeat { title, count } => {
            let count = usize::try_from(count).map_err(|_| {
                PlannedWorkoutError::Repository(
                    "stored planned workout repeat count cannot be negative".to_string(),
                )
            })?;
            Ok(PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat {
                title,
                count,
            }))
        }
        StoredPlannedWorkoutLineDocument::Step {
            duration_seconds,
            step_kind,
            target,
        } => Ok(PlannedWorkoutLine::Step(PlannedWorkoutStep {
            duration_seconds,
            kind: map_stored_step_kind(&step_kind)?,
            target: map_stored_target_to_domain(target),
        })),
    }
}

fn map_domain_target_to_stored_target(
    target: &PlannedWorkoutTarget,
) -> StoredPlannedWorkoutTargetDocument {
    match target {
        PlannedWorkoutTarget::PercentFtp { min, max } => {
            StoredPlannedWorkoutTargetDocument::PercentFtp {
                min: *min,
                max: *max,
            }
        }
        PlannedWorkoutTarget::WattsRange { min, max } => {
            StoredPlannedWorkoutTargetDocument::WattsRange {
                min: *min,
                max: *max,
            }
        }
    }
}

fn map_stored_target_to_domain(target: StoredPlannedWorkoutTargetDocument) -> PlannedWorkoutTarget {
    match target {
        StoredPlannedWorkoutTargetDocument::PercentFtp { min, max } => {
            PlannedWorkoutTarget::PercentFtp { min, max }
        }
        StoredPlannedWorkoutTargetDocument::WattsRange { min, max } => {
            PlannedWorkoutTarget::WattsRange { min, max }
        }
    }
}

fn map_step_kind_to_str(kind: &PlannedWorkoutStepKind) -> &'static str {
    match kind {
        PlannedWorkoutStepKind::Steady => "steady",
        PlannedWorkoutStepKind::Ramp => "ramp",
    }
}

fn map_stored_step_kind(value: &str) -> Result<PlannedWorkoutStepKind, PlannedWorkoutError> {
    match value {
        "steady" => Ok(PlannedWorkoutStepKind::Steady),
        "ramp" => Ok(PlannedWorkoutStepKind::Ramp),
        other => Err(PlannedWorkoutError::Repository(format!(
            "unknown stored planned workout step kind: {other}"
        ))),
    }
}

fn sort_planned_workouts(workouts: &mut [PlannedWorkout]) {
    workouts.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.planned_workout_id.cmp(&right.planned_workout_id))
    });
}

#[cfg(test)]
mod tests {
    use crate::domain::planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
        PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    };

    use super::{
        map_imported_document_to_domain, map_imported_workout_to_document, sort_planned_workouts,
        ImportedPlannedWorkoutDocument, StoredPlannedWorkoutContentDocument,
        StoredPlannedWorkoutLineDocument, StoredPlannedWorkoutTargetDocument,
    };

    #[test]
    fn imported_planned_workout_document_round_trip_preserves_fields() {
        let workout = PlannedWorkout::new(
            "planned-imported-1".to_string(),
            "user-1".to_string(),
            "2026-05-10".to_string(),
            PlannedWorkoutContent {
                lines: vec![
                    PlannedWorkoutLine::Text(PlannedWorkoutText {
                        text: "Warmup".to_string(),
                    }),
                    PlannedWorkoutLine::Step(PlannedWorkoutStep {
                        duration_seconds: 600,
                        kind: PlannedWorkoutStepKind::Steady,
                        target: PlannedWorkoutTarget::PercentFtp {
                            min: 55.0,
                            max: 65.0,
                        },
                    }),
                ],
            },
        );

        let mapped =
            map_imported_document_to_domain(map_imported_workout_to_document(&workout)).unwrap();

        assert_eq!(mapped, workout);
    }

    #[test]
    fn imported_planned_workout_document_rejects_unknown_step_kind() {
        let error = map_imported_document_to_domain(ImportedPlannedWorkoutDocument {
            user_id: "user-1".to_string(),
            planned_workout_id: "planned-1".to_string(),
            date: "2026-05-10".to_string(),
            workout: StoredPlannedWorkoutContentDocument {
                lines: vec![StoredPlannedWorkoutLineDocument::Step {
                    duration_seconds: 600,
                    step_kind: "mystery".to_string(),
                    target: StoredPlannedWorkoutTargetDocument::WattsRange { min: 180, max: 220 },
                }],
            },
        })
        .unwrap_err();

        assert!(matches!(error, PlannedWorkoutError::Repository(_)));
    }

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
}
