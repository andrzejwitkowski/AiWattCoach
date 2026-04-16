use futures::TryStreamExt;
use mongodb::{bson::doc, Collection};
use serde::Deserialize;

use crate::domain::{
    calendar::{BoxFuture as CalendarBoxFuture, CalendarError, HiddenCalendarEventSource},
    calendar_labels::{
        BoxFuture as CalendarLabelsBoxFuture, CalendarLabel, CalendarLabelError,
        CalendarLabelPayload, CalendarLabelSource, CalendarRaceLabel,
    },
    calendar_view::CalendarEntryRace,
    external_sync::ExternalSyncStatus,
    intervals::DateRange,
};

#[derive(Clone)]
pub struct MongoCalendarEntryViewCalendarSource {
    collection: Collection<CalendarEntryViewDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct CalendarEntryViewDocument {
    entry_id: String,
    date: String,
    title: String,
    subtitle: Option<String>,
    race_id: Option<String>,
    description: Option<String>,
    race: Option<CalendarEntryRaceDocument>,
    sync: Option<CalendarEntrySyncDocument>,
}

#[derive(Clone, Debug, Deserialize)]
struct CalendarEntryRaceDocument {
    distance_meters: i32,
    discipline: String,
    priority: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CalendarEntrySyncDocument {
    linked_intervals_event_id: Option<i64>,
    sync_status: Option<String>,
}

impl MongoCalendarEntryViewCalendarSource {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("calendar_entry_views"),
        }
    }

    async fn list_race_entries(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<CalendarEntryViewDocument>, CalendarLabelError> {
        self.collection
            .find(doc! {
                "user_id": user_id,
                "entry_kind": "race",
                "date": {
                    "$gte": &range.oldest,
                    "$lte": &range.newest,
                },
            })
            .sort(doc! { "date": 1, "entry_id": 1 })
            .await
            .map_err(|error| CalendarLabelError::Internal(error.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| CalendarLabelError::Internal(error.to_string()))
    }
}

impl CalendarLabelSource for MongoCalendarEntryViewCalendarSource {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarLabelsBoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let entries = source.list_race_entries(&user_id, &range).await?;
            entries.into_iter().map(map_race_label).collect()
        })
    }
}

impl HiddenCalendarEventSource for MongoCalendarEntryViewCalendarSource {
    fn list_hidden_intervals_event_ids(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> CalendarBoxFuture<Result<Vec<i64>, CalendarError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let entries = source
                .list_race_entries(&user_id, &range)
                .await
                .map_err(map_calendar_error)?;
            Ok(entries
                .into_iter()
                .filter_map(|entry| entry.sync.and_then(|sync| sync.linked_intervals_event_id))
                .collect())
        })
    }
}

fn map_race_label(
    document: CalendarEntryViewDocument,
) -> Result<CalendarLabel, CalendarLabelError> {
    let CalendarEntryViewDocument {
        entry_id,
        date,
        title,
        subtitle,
        race_id,
        description,
        race,
        sync,
    } = document;
    let race_id = race_id.ok_or_else(|| {
        CalendarLabelError::Internal(format!("calendar entry {entry_id} missing race_id"))
    })?;
    let sync = sync.unwrap_or(CalendarEntrySyncDocument {
        linked_intervals_event_id: None,
        sync_status: None,
    });
    let race = race
        .map(map_race_document)
        .or_else(|| parse_race_description(description.as_deref()).ok())
        .ok_or_else(|| {
            CalendarLabelError::Internal(format!(
                "calendar entry {entry_id} missing structured race metadata"
            ))
        })?;
    let race_name = strip_race_title_prefix(&title).to_string();

    Ok(CalendarLabel {
        label_key: entry_id,
        date: date.clone(),
        title,
        subtitle,
        payload: CalendarLabelPayload::Race(CalendarRaceLabel {
            race_id,
            date,
            name: race_name,
            distance_meters: race.distance_meters,
            discipline: race.discipline,
            priority: race.priority,
            sync_status: sync
                .sync_status
                .unwrap_or_else(|| ExternalSyncStatus::Pending.as_str().to_string()),
            linked_intervals_event_id: sync.linked_intervals_event_id,
        }),
    })
}

fn parse_race_description(
    description: Option<&str>,
) -> Result<CalendarEntryRace, CalendarLabelError> {
    let mut distance_meters = None;
    let mut discipline = None;
    let mut priority = None;

    for line in description.unwrap_or_default().lines() {
        if let Some(value) = line.strip_prefix("distance_meters=") {
            distance_meters = value.parse::<i32>().ok();
            continue;
        }
        if let Some(value) = line.strip_prefix("discipline=") {
            discipline = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("priority=") {
            priority = Some(value.to_string());
        }
    }

    Ok(CalendarEntryRace {
        distance_meters: distance_meters.ok_or_else(|| {
            CalendarLabelError::Internal("race calendar entry missing distance_meters".to_string())
        })?,
        discipline: discipline.ok_or_else(|| {
            CalendarLabelError::Internal("race calendar entry missing discipline".to_string())
        })?,
        priority: priority.ok_or_else(|| {
            CalendarLabelError::Internal("race calendar entry missing priority".to_string())
        })?,
    })
}

fn map_race_document(document: CalendarEntryRaceDocument) -> CalendarEntryRace {
    CalendarEntryRace {
        distance_meters: document.distance_meters,
        discipline: document.discipline,
        priority: document.priority,
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

fn strip_race_title_prefix(title: &str) -> &str {
    title.strip_prefix("Race ").unwrap_or(title)
}
