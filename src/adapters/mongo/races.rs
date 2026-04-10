use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::{
    calendar::{BoxFuture as CalendarBoxFuture, CalendarError, HiddenCalendarEventSource},
    calendar_labels::{
        CalendarLabel, CalendarLabelError, CalendarLabelPayload, CalendarLabelSource,
        CalendarRaceLabel,
    },
    intervals::DateRange,
    races::{
        BoxFuture as RaceBoxFuture, Race, RaceDiscipline, RaceError, RacePriority, RaceRepository,
        RaceResult, RaceSyncStatus,
    },
};

#[derive(Clone)]
pub struct MongoRaceRepository {
    collection: Collection<RaceDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RaceDocument {
    user_id: String,
    race_id: String,
    date: String,
    name: String,
    distance_meters: i32,
    discipline: String,
    priority: String,
    linked_intervals_event_id: Option<i64>,
    sync_status: String,
    synced_payload_hash: Option<String>,
    last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
    last_synced_at_epoch_seconds: Option<i64>,
}

impl MongoRaceRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client.database(database.as_ref()).collection("races"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), RaceError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "race_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("races_user_race_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("races_user_date".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "linked_intervals_event_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("races_user_linked_intervals_event".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| RaceError::Internal(error.to_string()))?;

        Ok(())
    }
}

impl RaceRepository for MongoRaceRepository {
    fn list_by_user_id(&self, user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                })
                .sort(doc! { "date": 1, "name": 1 })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_race)
                .collect()
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &range.oldest,
                        "$lte": &range.newest,
                    },
                })
                .sort(doc! { "date": 1, "name": 1 })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_race)
                .collect()
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! { "user_id": &user_id, "race_id": &race_id })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?
                .map(map_document_to_race)
                .transpose()
        })
    }

    fn upsert(&self, race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        let collection = self.collection.clone();
        let document = map_race_to_document(&race);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! { "user_id": &document.user_id, "race_id": &document.race_id },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?;
            Ok(race)
        })
    }

    fn delete(&self, user_id: &str, race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "user_id": &user_id, "race_id": &race_id })
                .await
                .map_err(|error| RaceError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}

impl CalendarLabelSource for MongoRaceRepository {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> crate::domain::calendar_labels::BoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>>
    {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let races = repository
                .list_by_user_id_and_range(&user_id, &range)
                .await
                .map_err(map_race_error_to_label_error)?;

            Ok(races
                .into_iter()
                .map(|race| CalendarLabel {
                    label_key: format!("race:{}", race.race_id),
                    date: race.date.clone(),
                    title: race.label_title(),
                    subtitle: Some(race.label_subtitle()),
                    payload: CalendarLabelPayload::Race(CalendarRaceLabel {
                        race_id: race.race_id,
                        date: race.date,
                        name: race.name,
                        distance_meters: race.distance_meters,
                        discipline: race.discipline.as_str().to_string(),
                        priority: race.priority.as_str().to_string(),
                        sync_status: race.sync_status.as_str().to_string(),
                        linked_intervals_event_id: race.linked_intervals_event_id,
                    }),
                })
                .collect())
        })
    }
}

impl HiddenCalendarEventSource for MongoRaceRepository {
    fn list_hidden_intervals_event_ids(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarBoxFuture<Result<Vec<i64>, CalendarError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let races = repository
                .list_by_user_id_and_range(&user_id, &range)
                .await
                .map_err(map_race_error_to_calendar_error)?;
            Ok(races
                .into_iter()
                .filter_map(|race| race.linked_intervals_event_id)
                .collect())
        })
    }
}

fn map_race_to_document(race: &Race) -> RaceDocument {
    RaceDocument {
        user_id: race.user_id.clone(),
        race_id: race.race_id.clone(),
        date: race.date.clone(),
        name: race.name.clone(),
        distance_meters: race.distance_meters,
        discipline: race.discipline.as_str().to_string(),
        priority: race.priority.as_str().to_string(),
        linked_intervals_event_id: race.linked_intervals_event_id,
        sync_status: race.sync_status.as_str().to_string(),
        synced_payload_hash: race.synced_payload_hash.clone(),
        last_error: race.last_error.clone(),
        result: race.result.as_ref().map(|r| r.as_str().to_string()),
        created_at_epoch_seconds: race.created_at_epoch_seconds,
        updated_at_epoch_seconds: race.updated_at_epoch_seconds,
        last_synced_at_epoch_seconds: race.last_synced_at_epoch_seconds,
    }
}

fn map_document_to_race(document: RaceDocument) -> Result<Race, RaceError> {
    Ok(Race {
        race_id: document.race_id,
        user_id: document.user_id,
        date: document.date,
        name: document.name,
        distance_meters: document.distance_meters,
        discipline: map_discipline(&document.discipline)?,
        priority: map_priority(&document.priority)?,
        linked_intervals_event_id: document.linked_intervals_event_id,
        sync_status: map_sync_status(&document.sync_status)?,
        synced_payload_hash: document.synced_payload_hash,
        last_error: document.last_error,
        result: document.result.as_deref().map(map_result).transpose()?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
        last_synced_at_epoch_seconds: document.last_synced_at_epoch_seconds,
    })
}

fn map_discipline(value: &str) -> Result<RaceDiscipline, RaceError> {
    match value {
        "road" => Ok(RaceDiscipline::Road),
        "mtb" => Ok(RaceDiscipline::Mtb),
        "gravel" => Ok(RaceDiscipline::Gravel),
        "cyclocross" => Ok(RaceDiscipline::Cyclocross),
        "timetrial" => Ok(RaceDiscipline::Timetrial),
        other => Err(RaceError::Internal(format!(
            "unknown race discipline: {other}"
        ))),
    }
}

fn map_priority(value: &str) -> Result<RacePriority, RaceError> {
    match value {
        "A" => Ok(RacePriority::A),
        "B" => Ok(RacePriority::B),
        "C" => Ok(RacePriority::C),
        other => Err(RaceError::Internal(format!(
            "unknown race priority: {other}"
        ))),
    }
}

fn map_sync_status(value: &str) -> Result<RaceSyncStatus, RaceError> {
    match value {
        "pending" => Ok(RaceSyncStatus::Pending),
        "synced" => Ok(RaceSyncStatus::Synced),
        "failed" => Ok(RaceSyncStatus::Failed),
        "pending_delete" => Ok(RaceSyncStatus::PendingDelete),
        other => Err(RaceError::Internal(format!(
            "unknown race sync status: {other}"
        ))),
    }
}

fn map_result(value: &str) -> Result<RaceResult, RaceError> {
    match value {
        "finished" => Ok(RaceResult::Finished),
        "dnf" => Ok(RaceResult::Dnf),
        "dsq" => Ok(RaceResult::Dsq),
        other => Err(RaceError::Internal(format!("unknown race result: {other}"))),
    }
}

fn map_race_error_to_label_error(error: RaceError) -> CalendarLabelError {
    match error {
        RaceError::Unauthenticated => CalendarLabelError::Unauthenticated,
        RaceError::Validation(message) => CalendarLabelError::Validation(message),
        RaceError::Unavailable(message) => CalendarLabelError::Unavailable(message),
        RaceError::Internal(message) => CalendarLabelError::Internal(message),
        RaceError::NotFound => CalendarLabelError::Internal("Race not found".to_string()),
    }
}

fn map_race_error_to_calendar_error(error: RaceError) -> CalendarError {
    match error {
        RaceError::Unauthenticated => CalendarError::Unauthenticated,
        RaceError::Validation(message) => CalendarError::Validation(message),
        RaceError::Unavailable(message) => CalendarError::Unavailable(message),
        RaceError::Internal(message) => CalendarError::Internal(message),
        RaceError::NotFound => CalendarError::Internal("Race not found".to_string()),
    }
}
