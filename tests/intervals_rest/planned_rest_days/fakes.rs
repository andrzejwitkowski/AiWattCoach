use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    calendar::{CalendarError, HiddenCalendarEventSource},
    calendar_labels::{
        CalendarLabel, CalendarLabelError, CalendarLabelPayload, CalendarLabelSource,
        CalendarPlannedRestDayLabel,
    },
    intervals::DateRange,
    planned_rest_days::{
        BoxFuture as PlannedRestDayBoxFuture, CreatePlannedRestDay, PlannedRestDay,
        PlannedRestDayError, PlannedRestDayUseCases, UpdatePlannedRestDay,
    },
    races::{BoxFuture as RaceBoxFuture, CreateRace, Race, RaceError, RaceUseCases, UpdateRace},
};

#[derive(Clone, Default)]
pub(crate) struct EmptyPlannedRestLabelSource;

impl CalendarLabelSource for EmptyPlannedRestLabelSource {
    fn list_labels(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar_labels::BoxFuture<
        Result<Vec<CalendarLabel>, CalendarLabelError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone)]
pub(crate) struct PlannedRestLabelSource {
    entries: Arc<Mutex<Vec<PlannedRestDay>>>,
}

impl PlannedRestLabelSource {
    pub(crate) fn with_entries(entries: Vec<PlannedRestDay>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(entries)),
        }
    }
}

impl CalendarLabelSource for PlannedRestLabelSource {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> aiwattcoach::domain::calendar_labels::BoxFuture<
        Result<Vec<CalendarLabel>, CalendarLabelError>,
    > {
        let source = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let entries = source.entries.lock().unwrap().clone();
            let mut labels = Vec::new();
            for entry in entries.into_iter().filter(|entry| entry.user_id == user_id) {
                let dates = aiwattcoach::domain::planned_rest_days::expand_inclusive_date_range(
                    &entry.start_date,
                    &entry.end_date,
                )
                .map_err(|error| CalendarLabelError::Internal(error.to_string()))?;
                for date in dates {
                    if date < range.oldest || date > range.newest {
                        continue;
                    }
                    labels.push(CalendarLabel {
                        label_key: format!("planned_rest_day:{}", entry.planned_rest_day_id),
                        date: date.clone(),
                        title: entry.display_title(),
                        subtitle: entry.label_subtitle_for_date(&date),
                        payload: CalendarLabelPayload::PlannedRestDay(
                            CalendarPlannedRestDayLabel {
                                planned_rest_day_id: entry.planned_rest_day_id.clone(),
                                start_date: entry.start_date.clone(),
                                end_date: entry.end_date.clone(),
                                title: entry.title.clone(),
                                note: entry.note.clone(),
                            },
                        ),
                    });
                }
            }
            Ok(labels)
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct RecordingPlannedRestDayService {
    entries: Arc<Mutex<Vec<PlannedRestDay>>>,
    next_id: Arc<Mutex<u32>>,
}

impl PlannedRestDayUseCases for RecordingPlannedRestDayService {
    fn list(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> PlannedRestDayBoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>> {
        let user_id = user_id.to_string();
        let range = range.clone();
        let entries = self.entries.lock().unwrap().clone();
        Box::pin(async move {
            Ok(entries
                .into_iter()
                .filter(|entry| {
                    entry.user_id == user_id
                        && entry.start_date <= range.newest
                        && entry.end_date >= range.oldest
                })
                .collect())
        })
    }

    fn get(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> PlannedRestDayBoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        let entries = self.entries.lock().unwrap().clone();
        Box::pin(async move {
            entries
                .into_iter()
                .find(|entry| {
                    entry.user_id == user_id && entry.planned_rest_day_id == planned_rest_day_id
                })
                .ok_or(PlannedRestDayError::NotFound)
        })
    }

    fn create(
        &self,
        user_id: &str,
        request: CreatePlannedRestDay,
    ) -> PlannedRestDayBoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let user_id = user_id.to_string();
        let entries = self.entries.clone();
        let next_id = self.next_id.clone();
        Box::pin(async move {
            let id = {
                let mut counter = next_id.lock().unwrap();
                let id = format!("prd-{}", *counter);
                *counter += 1;
                id
            };
            let entry = PlannedRestDay::new(
                id,
                user_id,
                request.start_date,
                request.end_date,
                request.title,
                request.note,
                1,
                1,
            )?;
            entries.lock().unwrap().push(entry.clone());
            Ok(entry)
        })
    }

    fn update(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
        request: UpdatePlannedRestDay,
    ) -> PlannedRestDayBoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        let entries = self.entries.clone();
        Box::pin(async move {
            let mut stored = entries.lock().unwrap();
            let existing = stored
                .iter()
                .find(|entry| {
                    entry.user_id == user_id && entry.planned_rest_day_id == planned_rest_day_id
                })
                .cloned()
                .ok_or(PlannedRestDayError::NotFound)?;
            let updated = existing.mark_updated(request, 2)?;
            stored.retain(|entry| entry.planned_rest_day_id != planned_rest_day_id);
            stored.push(updated.clone());
            Ok(updated)
        })
    }

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> PlannedRestDayBoxFuture<Result<(), PlannedRestDayError>> {
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        let entries = self.entries.clone();
        Box::pin(async move {
            let mut stored = entries.lock().unwrap();
            let before = stored.len();
            stored.retain(|entry| {
                !(entry.user_id == user_id && entry.planned_rest_day_id == planned_rest_day_id)
            });
            if stored.len() == before {
                return Err(PlannedRestDayError::NotFound);
            }
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct EmptyHiddenSource;

impl HiddenCalendarEventSource for EmptyHiddenSource {
    fn list_hidden_intervals_event_ids(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> aiwattcoach::domain::calendar::BoxFuture<Result<Vec<i64>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
pub(crate) struct StubRaceService;

impl RaceUseCases for StubRaceService {
    fn list_races(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_race(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::NotFound) })
    }

    fn create_race(
        &self,
        _user_id: &str,
        _request: CreateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::Internal("not configured".to_string())) })
    }

    fn update_race(
        &self,
        _user_id: &str,
        _race_id: &str,
        _request: UpdateRace,
    ) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async { Err(RaceError::NotFound) })
    }

    fn delete_race(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        Box::pin(async { Err(RaceError::NotFound) })
    }
}
