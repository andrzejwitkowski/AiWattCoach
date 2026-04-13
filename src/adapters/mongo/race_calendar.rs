use futures::TryStreamExt;
use mongodb::{bson::doc, Collection};
use serde::Deserialize;
use std::collections::HashMap;

use crate::domain::{
    calendar::{BoxFuture as CalendarBoxFuture, CalendarError, HiddenCalendarEventSource},
    calendar_labels::{
        BoxFuture as CalendarLabelsBoxFuture, CalendarLabel, CalendarLabelError,
        CalendarLabelPayload, CalendarLabelSource, CalendarRaceLabel,
    },
    external_sync::ExternalSyncStatus,
    intervals::DateRange,
    races::{Race, RaceDiscipline, RaceError, RacePriority, RaceResult},
};

#[derive(Clone)]
pub struct MongoRaceCalendarSource {
    race_collection: Collection<RaceDocument>,
    sync_collection: Collection<ExternalSyncStateDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct RaceDocument {
    user_id: String,
    race_id: String,
    date: String,
    name: String,
    distance_meters: i32,
    discipline: String,
    priority: String,
    result: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct ExternalSyncStateDocument {
    canonical_entity_id: String,
    external_id: Option<String>,
    sync_status: String,
}

impl MongoRaceCalendarSource {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        let database = client.database(database.as_ref());
        Self {
            race_collection: database.collection("races"),
            sync_collection: database.collection("external_sync_states"),
        }
    }
}

impl CalendarLabelSource for MongoRaceCalendarSource {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarLabelsBoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let races = source
                .list_races(&user_id, &range)
                .await
                .map_err(map_race_error)?;
            let sync_states_by_race_id =
                sync_states_by_race_id(source.list_race_sync_states(&user_id).await?);

            Ok(races
                .into_iter()
                .map(|race| {
                    let sync_state = sync_states_by_race_id.get(&race.race_id);
                    CalendarLabel {
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
                            sync_status: sync_state
                                .map(|state| {
                                    map_sync_status(&state.sync_status).as_str().to_string()
                                })
                                .unwrap_or_else(|| {
                                    ExternalSyncStatus::Pending.as_str().to_string()
                                }),
                            linked_intervals_event_id: sync_state
                                .and_then(|state| state.external_id.as_deref())
                                .and_then(|value| value.parse::<i64>().ok()),
                        }),
                    }
                })
                .collect())
        })
    }
}

impl HiddenCalendarEventSource for MongoRaceCalendarSource {
    fn list_hidden_intervals_event_ids(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarBoxFuture<Result<Vec<i64>, CalendarError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let races = source
                .list_races(&user_id, &range)
                .await
                .map_err(map_calendar_from_race_error)?;
            let sync_states_by_race_id = sync_states_by_race_id(
                source
                    .list_race_sync_states(&user_id)
                    .await
                    .map_err(map_calendar_error)?,
            );
            Ok(races
                .into_iter()
                .filter_map(|race| sync_states_by_race_id.get(&race.race_id))
                .filter_map(|state| state.external_id.clone())
                .filter_map(|value| value.parse::<i64>().ok())
                .collect())
        })
    }
}

fn sync_states_by_race_id(
    sync_states: Vec<ExternalSyncStateDocument>,
) -> HashMap<String, ExternalSyncStateDocument> {
    sync_states
        .into_iter()
        .map(|state| (state.canonical_entity_id.clone(), state))
        .collect()
}

fn map_calendar_from_race_error(error: RaceError) -> CalendarError {
    match error {
        RaceError::Unauthenticated => CalendarError::Unauthenticated,
        RaceError::Validation(message) => CalendarError::Validation(message),
        RaceError::Unavailable(message) => CalendarError::Unavailable(message),
        RaceError::Internal(message) => CalendarError::Internal(message),
        RaceError::NotFound => CalendarError::Internal("Race not found".to_string()),
    }
}

impl MongoRaceCalendarSource {
    async fn list_races(&self, user_id: &str, range: &DateRange) -> Result<Vec<Race>, RaceError> {
        self.race_collection
            .find(doc! {
                "user_id": user_id,
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
    }

    async fn list_race_sync_states(
        &self,
        user_id: &str,
    ) -> Result<Vec<ExternalSyncStateDocument>, CalendarLabelError> {
        self.sync_collection
            .find(doc! {
                "user_id": user_id,
                "provider": "intervals",
                "canonical_entity_kind": "race",
            })
            .await
            .map_err(|error| CalendarLabelError::Internal(error.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| CalendarLabelError::Internal(error.to_string()))
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
        result: document.result.as_deref().map(map_result).transpose()?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
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

fn map_result(value: &str) -> Result<RaceResult, RaceError> {
    match value {
        "finished" => Ok(RaceResult::Finished),
        "dnf" => Ok(RaceResult::Dnf),
        "dsq" => Ok(RaceResult::Dsq),
        other => Err(RaceError::Internal(format!("unknown race result: {other}"))),
    }
}

fn map_sync_status(value: &str) -> ExternalSyncStatus {
    match value {
        "synced" => ExternalSyncStatus::Synced,
        "failed" => ExternalSyncStatus::Failed,
        "pending_delete" => ExternalSyncStatus::PendingDelete,
        _ => ExternalSyncStatus::Pending,
    }
}

fn map_race_error(error: RaceError) -> CalendarLabelError {
    match error {
        RaceError::Unauthenticated => CalendarLabelError::Unauthenticated,
        RaceError::Validation(message) => CalendarLabelError::Validation(message),
        RaceError::Unavailable(message) => CalendarLabelError::Unavailable(message),
        RaceError::Internal(message) => CalendarLabelError::Internal(message),
        RaceError::NotFound => CalendarLabelError::Internal("Race not found".to_string()),
    }
}

fn map_calendar_error(error: CalendarLabelError) -> CalendarError {
    match error {
        CalendarLabelError::Unauthenticated => CalendarError::Unauthenticated,
        CalendarLabelError::Validation(message) => CalendarError::Validation(message),
        CalendarLabelError::Unavailable(message) => CalendarError::Unavailable(message),
        CalendarLabelError::Internal(message) => CalendarError::Internal(message),
    }
}
