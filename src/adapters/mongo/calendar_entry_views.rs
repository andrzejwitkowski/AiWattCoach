use std::time::{SystemTime, UNIX_EPOCH};

use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::calendar_view::{
    BoxFuture as CalendarEntryViewBoxFuture, CalendarEntryKind, CalendarEntryRace,
    CalendarEntrySummary, CalendarEntrySync, CalendarEntryView, CalendarEntryViewError,
    CalendarEntryViewRepository,
};

#[derive(Clone)]
pub struct MongoCalendarEntryViewRepository {
    collection: Collection<CalendarEntryViewDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CalendarEntryViewDocument {
    user_id: String,
    entry_id: String,
    rewrite_gen: String,
    entry_kind: String,
    date: String,
    start_date_local: Option<String>,
    title: String,
    subtitle: Option<String>,
    description: Option<String>,
    raw_workout_doc: Option<String>,
    planned_workout_id: Option<String>,
    completed_workout_id: Option<String>,
    race_id: Option<String>,
    special_day_id: Option<String>,
    race: Option<CalendarEntryRaceDocument>,
    summary: Option<CalendarEntrySummaryDocument>,
    sync: Option<CalendarEntrySyncDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CalendarEntryRaceDocument {
    distance_meters: i32,
    discipline: String,
    priority: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CalendarEntrySummaryDocument {
    training_stress_score: Option<i32>,
    intensity_factor: Option<f64>,
    normalized_power_watts: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CalendarEntrySyncDocument {
    linked_intervals_event_id: Option<i64>,
    sync_status: Option<String>,
}

impl MongoCalendarEntryViewRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("calendar_entry_views"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CalendarEntryViewError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "entry_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("calendar_entry_views_user_entry_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("calendar_entry_views_user_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "entry_kind": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("calendar_entry_views_user_kind_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl CalendarEntryViewRepository for MongoCalendarEntryViewRepository {
    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CalendarEntryViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &oldest,
                        "$lte": &newest,
                    },
                })
                .sort(doc! { "date": 1, "entry_kind": 1, "entry_id": 1 })
                .await
                .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_entry)
                .collect()
        })
    }

    fn upsert(
        &self,
        entry: CalendarEntryView,
    ) -> CalendarEntryViewBoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let collection = self.collection.clone();
        let document = map_entry_to_document(&entry, &new_rewrite_generation());
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "entry_id": &document.entry_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;
            Ok(entry)
        })
    }

    fn replace_all_for_user(
        &self,
        user_id: &str,
        entries: Vec<CalendarEntryView>,
    ) -> CalendarEntryViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let rewrite_gen = new_rewrite_generation();
        let documents = entries
            .iter()
            .map(|entry| {
                if entry.user_id != user_id {
                    return Err(CalendarEntryViewError::Repository(format!(
                        "calendar entry user mismatch for replace_all_for_user: expected {user_id}, got {}",
                        entry.user_id
                    )));
                }
                Ok(map_entry_to_document(entry, &rewrite_gen))
            })
            .collect::<Result<Vec<_>, _>>();
        Box::pin(async move {
            let documents = documents?;
            for document in &documents {
                collection
                    .replace_one(
                        doc! {
                            "user_id": &document.user_id,
                            "entry_id": &document.entry_id,
                        },
                        document,
                    )
                    .upsert(true)
                    .await
                    .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;
            }

            let retained_entry_ids = documents
                .iter()
                .map(|document| document.entry_id.clone())
                .collect::<Vec<_>>();

            let delete_filter = if retained_entry_ids.is_empty() {
                doc! { "user_id": &user_id, "rewrite_gen": { "$ne": &rewrite_gen } }
            } else {
                doc! {
                    "user_id": &user_id,
                    "entry_id": { "$nin": retained_entry_ids },
                    "rewrite_gen": { "$ne": &rewrite_gen },
                }
            };
            collection
                .delete_many(delete_filter)
                .await
                .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;

            Ok(entries)
        })
    }

    fn replace_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        entries: Vec<CalendarEntryView>,
    ) -> CalendarEntryViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let rewrite_gen = new_rewrite_generation();
        let documents = entries
            .iter()
            .map(|entry| {
                if entry.user_id != user_id {
                    return Err(CalendarEntryViewError::Repository(format!(
                        "calendar entry user mismatch for replace_range_for_user: expected {user_id}, got {}",
                        entry.user_id
                    )));
                }
                Ok(map_entry_to_document(entry, &rewrite_gen))
            })
            .collect::<Result<Vec<_>, _>>();
        Box::pin(async move {
            let documents = documents?;
            let incoming_entry_ids = documents
                .iter()
                .map(|document| document.entry_id.clone())
                .collect::<Vec<_>>();

            for document in &documents {
                collection
                    .replace_one(
                        doc! {
                            "user_id": &document.user_id,
                            "entry_id": &document.entry_id,
                        },
                        document,
                    )
                    .upsert(true)
                    .await
                    .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;
            }

            let delete_filter = if incoming_entry_ids.is_empty() {
                doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &oldest,
                        "$lte": &newest,
                    },
                    "rewrite_gen": { "$ne": &rewrite_gen },
                }
            } else {
                doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &oldest,
                        "$lte": &newest,
                    },
                    "entry_id": { "$nin": &incoming_entry_ids },
                    "rewrite_gen": { "$ne": &rewrite_gen },
                }
            };

            collection
                .delete_many(delete_filter)
                .await
                .map_err(|error| CalendarEntryViewError::Repository(error.to_string()))?;

            Ok(entries)
        })
    }
}

fn map_entry_to_document(
    entry: &CalendarEntryView,
    rewrite_gen: &str,
) -> CalendarEntryViewDocument {
    CalendarEntryViewDocument {
        user_id: entry.user_id.clone(),
        entry_id: entry.entry_id.clone(),
        rewrite_gen: rewrite_gen.to_string(),
        entry_kind: entry.entry_kind.as_str().to_string(),
        date: entry.date.clone(),
        start_date_local: entry.start_date_local.clone(),
        title: entry.title.clone(),
        subtitle: entry.subtitle.clone(),
        description: entry.description.clone(),
        raw_workout_doc: entry.raw_workout_doc.clone(),
        planned_workout_id: entry.planned_workout_id.clone(),
        completed_workout_id: entry.completed_workout_id.clone(),
        race_id: entry.race_id.clone(),
        special_day_id: entry.special_day_id.clone(),
        race: entry.race.as_ref().map(|race| CalendarEntryRaceDocument {
            distance_meters: race.distance_meters,
            discipline: race.discipline.clone(),
            priority: race.priority.clone(),
        }),
        summary: entry
            .summary
            .as_ref()
            .map(|summary| CalendarEntrySummaryDocument {
                training_stress_score: summary.training_stress_score,
                intensity_factor: summary.intensity_factor,
                normalized_power_watts: summary.normalized_power_watts,
            }),
        sync: entry.sync.as_ref().map(|sync| CalendarEntrySyncDocument {
            linked_intervals_event_id: sync.linked_intervals_event_id,
            sync_status: sync.sync_status.clone(),
        }),
    }
}

fn map_document_to_entry(
    document: CalendarEntryViewDocument,
) -> Result<CalendarEntryView, CalendarEntryViewError> {
    Ok(CalendarEntryView {
        entry_id: document.entry_id,
        user_id: document.user_id,
        entry_kind: map_kind_from_str(&document.entry_kind)?,
        date: document.date,
        start_date_local: document.start_date_local,
        title: document.title,
        subtitle: document.subtitle,
        description: document.description,
        raw_workout_doc: document.raw_workout_doc,
        planned_workout_id: document.planned_workout_id,
        completed_workout_id: document.completed_workout_id,
        race_id: document.race_id,
        special_day_id: document.special_day_id,
        race: document.race.map(|race| CalendarEntryRace {
            distance_meters: race.distance_meters,
            discipline: race.discipline,
            priority: race.priority,
        }),
        summary: document.summary.map(|summary| CalendarEntrySummary {
            training_stress_score: summary.training_stress_score,
            intensity_factor: summary.intensity_factor,
            normalized_power_watts: summary.normalized_power_watts,
        }),
        sync: document.sync.map(|sync| CalendarEntrySync {
            linked_intervals_event_id: sync.linked_intervals_event_id,
            sync_status: sync.sync_status,
        }),
    })
}

fn new_rewrite_generation() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn map_kind_from_str(value: &str) -> Result<CalendarEntryKind, CalendarEntryViewError> {
    match value {
        "planned_workout" => Ok(CalendarEntryKind::PlannedWorkout),
        "completed_workout" => Ok(CalendarEntryKind::CompletedWorkout),
        "race" => Ok(CalendarEntryKind::Race),
        "special_day" => Ok(CalendarEntryKind::SpecialDay),
        other => Err(CalendarEntryViewError::Repository(format!(
            "unknown calendar entry kind: {other}"
        ))),
    }
}
