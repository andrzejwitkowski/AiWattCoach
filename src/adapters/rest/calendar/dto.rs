use serde::{Deserialize, Serialize};

use crate::adapters::rest::intervals::EventDefinitionDto;

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ListCalendarEventsQuery {
    pub oldest: String,
    pub newest: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct SyncPlannedWorkoutPath {
    pub operation_key: String,
    pub date: String,
}

#[derive(Serialize)]
pub(super) struct CalendarLabelsResponseDto {
    #[serde(rename = "labelsByDate")]
    pub labels_by_date:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, CalendarLabelDto>>,
}

#[derive(Serialize)]
pub(super) struct CalendarLabelDto {
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub payload: CalendarLabelPayloadDto,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum CalendarLabelPayloadDto {
    Race(CalendarRaceLabelDto),
    Activity(CalendarActivityLabelDto),
    Health(CalendarHealthLabelDto),
    Custom(CalendarCustomLabelDto),
}

#[derive(Serialize)]
pub(super) struct CalendarRaceLabelDto {
    #[serde(rename = "raceId")]
    pub race_id: String,
    pub date: String,
    pub name: String,
    #[serde(rename = "distanceMeters")]
    pub distance_meters: i32,
    pub discipline: String,
    pub priority: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: String,
    #[serde(rename = "linkedIntervalsEventId")]
    pub linked_intervals_event_id: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct CalendarActivityLabelDto {
    #[serde(rename = "labelId")]
    pub label_id: String,
    #[serde(rename = "activityKind")]
    pub activity_kind: String,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CalendarHealthLabelDto {
    #[serde(rename = "labelId")]
    pub label_id: String,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CalendarCustomLabelDto {
    #[serde(rename = "labelId")]
    pub label_id: String,
    pub value: String,
}

#[derive(Serialize)]
pub(super) struct CalendarEventDto {
    pub id: i64,
    #[serde(rename = "calendarEntryId")]
    pub calendar_entry_id: String,
    #[serde(rename = "startDateLocal")]
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
    pub indoor: bool,
    pub color: Option<String>,
    #[serde(rename = "eventDefinition")]
    pub event_definition: EventDefinitionDto,
    #[serde(rename = "actualWorkout")]
    pub actual_workout: Option<serde_json::Value>,
    #[serde(rename = "plannedSource")]
    pub planned_source: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<String>,
    #[serde(rename = "linkedIntervalsEventId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_intervals_event_id: Option<i64>,
    #[serde(rename = "projectedWorkout")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projected_workout: Option<ProjectedWorkoutDto>,
}

#[derive(Serialize)]
pub(super) struct ProjectedWorkoutDto {
    #[serde(rename = "projectedWorkoutId")]
    pub projected_workout_id: String,
    #[serde(rename = "operationKey")]
    pub operation_key: String,
    pub date: String,
    #[serde(rename = "sourceWorkoutId")]
    pub source_workout_id: String,
}
