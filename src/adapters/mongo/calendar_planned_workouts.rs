use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson},
    Collection,
};
use serde::Deserialize;

use crate::adapters::mongo::training_plan_shared::{
    map_document_to_planned_workout, PlannedWorkoutDocument,
};
use crate::domain::calendar_view::{
    BoxFuture, CalendarPlannedSyncKey, CalendarPlannedWorkoutCandidate,
    CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
};
use crate::domain::planned_workouts::{
    PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutError, PlannedWorkoutLine,
    PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
    PlannedWorkoutText,
};

#[derive(Clone)]
pub struct MongoCalendarPlannedWorkoutSource {
    projected_collection: Collection<ProjectedPlannedWorkoutDocument>,
    snapshot_collection: Collection<TrainingPlanSnapshotLookupDocument>,
    imported_collection: Collection<ImportedPlannedWorkoutDocument>,
    sync_state_collection: Collection<ExternalSyncStateDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct ProjectedPlannedWorkoutDocument {
    user_id: String,
    operation_key: String,
    date: String,
    #[serde(default)]
    rest_day: bool,
    #[serde(default)]
    rest_day_reason: Option<String>,
    workout: Option<PlannedWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct TrainingPlanSnapshotLookupDocument {
    operation_key: String,
    start_date: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ImportedPlannedWorkoutDocument {
    user_id: String,
    planned_workout_id: String,
    date: String,
    #[serde(default)]
    rest_day: bool,
    #[serde(default)]
    rest_day_reason: Option<String>,
    name: Option<String>,
    description: Option<String>,
    event_type: Option<String>,
    workout: StoredPlannedWorkoutContentDocument,
}

#[derive(Clone, Debug, Deserialize)]
struct StoredPlannedWorkoutContentDocument {
    lines: Vec<StoredPlannedWorkoutLineDocument>,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredPlannedWorkoutTargetDocument {
    PercentFtp { min: f64, max: f64 },
    WattsRange { min: i32, max: i32 },
}

#[derive(Clone, Debug, Deserialize)]
struct ExternalSyncStateDocument {
    provider: String,
    canonical_entity_id: String,
    external_id: Option<String>,
}

impl MongoCalendarPlannedWorkoutSource {
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
            sync_state_collection: client
                .database(database.as_ref())
                .collection("external_sync_states"),
        }
    }
}

impl CalendarPlannedWorkoutSource for MongoCalendarPlannedWorkoutSource {
    fn list_candidates_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarPlannedWorkoutCandidate>, PlannedWorkoutError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let mut candidates =
                load_projected_candidates(&source, &user_id, &oldest, &newest).await?;
            candidates.extend(load_imported_candidates(&source, &user_id, &oldest, &newest).await?);

            let canonical_entity_ids = candidates
                .iter()
                .map(|candidate| candidate.workout.planned_workout_id.clone())
                .collect::<Vec<_>>();
            let sync_keys_by_entity =
                load_sync_keys_by_entity(&source, &user_id, &canonical_entity_ids).await?;

            for candidate in &mut candidates {
                candidate.sync_keys = sync_keys_by_entity
                    .get(&candidate.workout.planned_workout_id)
                    .cloned()
                    .unwrap_or_default();
            }

            candidates.sort_by(|left, right| {
                left.workout.date.cmp(&right.workout.date).then_with(|| {
                    left.workout
                        .planned_workout_id
                        .cmp(&right.workout.planned_workout_id)
                })
            });
            Ok(candidates)
        })
    }
}

async fn load_projected_candidates(
    source: &MongoCalendarPlannedWorkoutSource,
    user_id: &str,
    oldest: &str,
    newest: &str,
) -> Result<Vec<CalendarPlannedWorkoutCandidate>, PlannedWorkoutError> {
    let documents = source
        .projected_collection
        .find(doc! {
            "user_id": user_id,
            "superseded_at_epoch_seconds": Bson::Null,
            "$or": [
                { "workout": { "$ne": Bson::Null } },
                { "rest_day": true },
            ],
            "date": { "$gte": oldest, "$lte": newest },
        })
        .sort(doc! { "date": 1, "operation_key": 1 })
        .await
        .map_err(storage_error)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(storage_error)?;

    let operation_keys = documents
        .iter()
        .map(|document| document.operation_key.as_str())
        .collect::<Vec<_>>();
    let snapshot_start_dates = load_snapshot_start_dates(source, user_id, &operation_keys).await?;

    documents
        .into_iter()
        .filter(|document| {
            snapshot_start_dates
                .get(&document.operation_key)
                .map(|start_date| document.date >= *start_date)
                .unwrap_or(false)
        })
        .map(|document| {
            Ok(CalendarPlannedWorkoutCandidate {
                workout: map_projected_document_to_domain(document)?,
                origin: CalendarPlannedWorkoutOrigin::Projected,
                sync_keys: Vec::new(),
            })
        })
        .collect()
}

async fn load_imported_candidates(
    source: &MongoCalendarPlannedWorkoutSource,
    user_id: &str,
    oldest: &str,
    newest: &str,
) -> Result<Vec<CalendarPlannedWorkoutCandidate>, PlannedWorkoutError> {
    source
        .imported_collection
        .find(doc! {
            "user_id": user_id,
            "date": { "$gte": oldest, "$lte": newest },
        })
        .sort(doc! { "date": 1, "planned_workout_id": 1 })
        .await
        .map_err(storage_error)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(storage_error)?
        .into_iter()
        .map(|document| {
            Ok(CalendarPlannedWorkoutCandidate {
                workout: map_imported_document_to_domain(document)?,
                origin: CalendarPlannedWorkoutOrigin::Imported,
                sync_keys: Vec::new(),
            })
        })
        .collect()
}

async fn load_snapshot_start_dates(
    source: &MongoCalendarPlannedWorkoutSource,
    user_id: &str,
    operation_keys: &[&str],
) -> Result<std::collections::HashMap<String, String>, PlannedWorkoutError> {
    if operation_keys.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    source
        .snapshot_collection
        .find(doc! { "user_id": user_id, "operation_key": { "$in": operation_keys } })
        .await
        .map_err(storage_error)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(storage_error)?
        .into_iter()
        .map(|snapshot| Ok((snapshot.operation_key, snapshot.start_date)))
        .collect()
}

async fn load_sync_keys_by_entity(
    source: &MongoCalendarPlannedWorkoutSource,
    user_id: &str,
    canonical_entity_ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<CalendarPlannedSyncKey>>, PlannedWorkoutError> {
    if canonical_entity_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let documents = source
        .sync_state_collection
        .find(doc! {
            "user_id": user_id,
            "canonical_entity_kind": "planned_workout",
            "canonical_entity_id": { "$in": canonical_entity_ids },
        })
        .await
        .map_err(storage_error)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(storage_error)?;

    let mut sync_keys_by_entity =
        std::collections::HashMap::<String, Vec<CalendarPlannedSyncKey>>::new();
    for document in documents {
        let Some(external_id) = document.external_id else {
            continue;
        };
        sync_keys_by_entity
            .entry(document.canonical_entity_id)
            .or_default()
            .push(CalendarPlannedSyncKey {
                provider: document.provider,
                external_id,
            });
    }
    Ok(sync_keys_by_entity)
}

fn map_projected_document_to_domain(
    document: ProjectedPlannedWorkoutDocument,
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
    )
    .with_event_metadata(None, None, Some("Ride".to_string())))
}

fn map_imported_document_to_domain(
    document: ImportedPlannedWorkoutDocument,
) -> Result<PlannedWorkout, PlannedWorkoutError> {
    let planned_workout = PlannedWorkout::new(
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
    )
    .with_event_metadata(document.name, document.description, document.event_type);

    if document.rest_day {
        Ok(planned_workout.as_rest_day(document.rest_day_reason))
    } else {
        Ok(planned_workout)
    }
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
            kind: match step_kind.as_str() {
                "steady" => PlannedWorkoutStepKind::Steady,
                "ramp" => PlannedWorkoutStepKind::Ramp,
                other => {
                    return Err(PlannedWorkoutError::Repository(format!(
                        "unknown stored planned workout step kind: {other}"
                    )))
                }
            },
            target: match target {
                StoredPlannedWorkoutTargetDocument::PercentFtp { min, max } => {
                    PlannedWorkoutTarget::PercentFtp { min, max }
                }
                StoredPlannedWorkoutTargetDocument::WattsRange { min, max } => {
                    PlannedWorkoutTarget::WattsRange { min, max }
                }
            },
        })),
    }
}

fn storage_error(error: mongodb::error::Error) -> PlannedWorkoutError {
    PlannedWorkoutError::Repository(error.to_string())
}
