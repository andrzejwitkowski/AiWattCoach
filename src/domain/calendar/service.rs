use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    identity::Clock,
    intervals::{
        CreateEvent, DateRange, Event, EventCategory, IntervalsError, IntervalsUseCases,
        UpdateEvent,
    },
    training_plan::{TrainingPlanProjectedDay, TrainingPlanProjectionRepository},
};

use super::{
    BoxFuture, CalendarError, CalendarEvent, CalendarEventSource, CalendarProjectedWorkout,
    CalendarUseCases, HiddenCalendarEventSource, PlannedWorkoutSyncRecord,
    PlannedWorkoutSyncRepository, PlannedWorkoutSyncStatus, SyncPlannedWorkout,
};

#[derive(Clone)]
pub struct CalendarService<
    Intervals,
    Projections,
    Syncs,
    Hidden,
    Time,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Intervals: IntervalsUseCases + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Syncs: PlannedWorkoutSyncRepository + Clone + 'static,
    Hidden: HiddenCalendarEventSource + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    intervals: Intervals,
    projections: Projections,
    syncs: Syncs,
    hidden_event_source: Hidden,
    clock: Time,
    refresh: Refresh,
}

impl<Intervals, Projections, Syncs, Hidden, Time>
    CalendarService<Intervals, Projections, Syncs, Hidden, Time>
where
    Intervals: IntervalsUseCases + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Hidden: HiddenCalendarEventSource + Clone,
    Time: Clock + Clone,
{
    pub fn new(
        intervals: Intervals,
        projections: Projections,
        syncs: Syncs,
        hidden_event_source: Hidden,
        clock: Time,
    ) -> Self {
        Self {
            intervals,
            projections,
            syncs,
            hidden_event_source,
            clock,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Intervals, Projections, Syncs, Hidden, Time, Refresh>
    CalendarService<Intervals, Projections, Syncs, Hidden, Time, Refresh>
where
    Intervals: IntervalsUseCases + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Hidden: HiddenCalendarEventSource + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> CalendarService<Intervals, Projections, Syncs, Hidden, Time, NewRefresh>
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone,
    {
        CalendarService {
            intervals: self.intervals,
            projections: self.projections,
            syncs: self.syncs,
            hidden_event_source: self.hidden_event_source,
            clock: self.clock,
            refresh,
        }
    }

    async fn list_events_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<CalendarEvent>, CalendarError> {
        let intervals_events = self
            .intervals
            .list_events(user_id, range)
            .await
            .map_err(map_intervals_error)?;
        let projected_days = self
            .projections
            .list_active_by_user_id(user_id)
            .await
            .map_err(map_training_plan_error)?;
        let sync_records = self.syncs.list_by_user_id_and_range(user_id, range).await?;
        let hidden_linked_intervals_event_ids = self
            .hidden_event_source
            .list_hidden_intervals_event_ids(user_id, range)
            .await?;

        let syncs_by_projection = sync_records
            .into_iter()
            .map(|record| ((record.operation_key.clone(), record.date.clone()), record))
            .collect::<HashMap<_, _>>();
        let intervals_events_by_id = intervals_events
            .iter()
            .cloned()
            .map(|event| (event.id, event))
            .collect::<HashMap<_, _>>();

        let mut hidden_intervals_event_ids = hidden_linked_intervals_event_ids
            .into_iter()
            .collect::<HashSet<_>>();
        let mut merged = projected_days
            .into_iter()
            .filter(|day| is_date_in_range(&day.date, range))
            .filter(|day| !day.rest_day && day.workout.is_some())
            .map(|day| {
                let key = (day.operation_key.clone(), day.date.clone());
                let sync_record = syncs_by_projection.get(&key);
                if let Some(intervals_event_id) =
                    sync_record.and_then(|record| record.intervals_event_id)
                {
                    if intervals_events_by_id.contains_key(&intervals_event_id) {
                        hidden_intervals_event_ids.insert(intervals_event_id);
                    }
                }

                build_projected_calendar_event(day, sync_record)
            })
            .collect::<Vec<_>>();

        merged.extend(
            intervals_events
                .into_iter()
                .filter(|event| !hidden_intervals_event_ids.contains(&event.id))
                .map(|event| CalendarEvent {
                    calendar_entry_id: format!("intervals:{}", event.id),
                    event,
                    source: CalendarEventSource::Intervals,
                    projected_workout: None,
                    sync_status: None,
                    linked_intervals_event_id: None,
                }),
        );

        merged.sort_by(|left, right| {
            left.event
                .start_date_local
                .cmp(&right.event.start_date_local)
                .then_with(|| left.event.id.cmp(&right.event.id))
        });

        Ok(merged)
    }

    async fn sync_planned_workout_impl(
        &self,
        user_id: &str,
        request: SyncPlannedWorkout,
    ) -> Result<CalendarEvent, CalendarError> {
        let projected_day = self
            .projections
            .find_active_by_user_id_and_operation_key(user_id, &request.operation_key)
            .await
            .map_err(map_training_plan_error)?
            .into_iter()
            .find(|day| day.date == request.date)
            .ok_or(CalendarError::NotFound)?;

        if projected_day.rest_day || projected_day.workout.is_none() {
            return Err(CalendarError::Validation(
                "Only planned workout days can be synchronized".to_string(),
            ));
        }

        let payload_hash = projected_day_payload_hash(&projected_day);
        let now = self.clock.now_epoch_seconds();
        let sync_record = self
            .syncs
            .find_by_user_id_and_projection(user_id, &request.operation_key, &request.date)
            .await?
            .unwrap_or_else(|| {
                PlannedWorkoutSyncRecord::pending(
                    user_id.to_string(),
                    request.operation_key.clone(),
                    request.date.clone(),
                    projected_day.workout_id.clone(),
                    now,
                )
            });

        let pending_record = self
            .syncs
            .upsert(sync_record.mark_pending(projected_day.workout_id.clone(), now))
            .await?;

        let sync_result = async {
            let existing_remote_event = if let Some(intervals_event_id) =
                pending_record.intervals_event_id
            {
                match self.intervals.get_event(user_id, intervals_event_id).await {
                    Ok(event) => Some(event),
                    Err(IntervalsError::NotFound) => None,
                    Err(error) => return Err(map_intervals_error(error)),
                }
            } else {
                find_existing_remote_event(&self.intervals, user_id, &projected_day, &payload_hash)
                    .await?
            };

            let remote_event = if let Some(existing_remote_event) = existing_remote_event {
                self.intervals
                    .update_event(
                        user_id,
                        existing_remote_event.id,
                        build_update_event(&projected_day, &existing_remote_event),
                    )
                    .await
                    .map_err(map_intervals_error)?
            } else {
                self.intervals
                    .create_event(user_id, build_create_event(&projected_day))
                    .await
                    .map_err(map_intervals_error)?
            };

            Ok(remote_event)
        }
        .await;

        match sync_result {
            Ok(remote_event) => {
                let synced_record = self
                    .syncs
                    .upsert(pending_record.mark_synced(
                        remote_event.id,
                        projected_day.workout_id.clone(),
                        payload_hash,
                        self.clock.now_epoch_seconds(),
                    ))
                    .await?;
                if let Err(error) = self
                    .refresh
                    .refresh_range_for_user(user_id, &request.date, &request.date)
                    .await
                {
                    tracing::warn!(
                        %user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        %error,
                        "planned workout sync succeeded but calendar view refresh failed"
                    );
                }
                Ok(build_projected_calendar_event(
                    projected_day,
                    Some(&synced_record),
                ))
            }
            Err(error) => {
                let sync_action = if pending_record.intervals_event_id.is_some() {
                    "update"
                } else {
                    "create"
                };
                tracing::warn!(
                    user_id,
                    operation_key = %request.operation_key,
                    date = %request.date,
                    sync_action,
                    linked_intervals_event_id = pending_record.intervals_event_id,
                    payload_hash = %payload_hash,
                    workout_name = projected_workout_name(&projected_day).as_deref().unwrap_or_default(),
                    error = %error,
                    "planned workout sync failed"
                );
                let failed_record = pending_record.mark_failed(
                    projected_day.workout_id.clone(),
                    error.to_string(),
                    self.clock.now_epoch_seconds(),
                );
                if let Err(persist_error) = self.syncs.upsert(failed_record).await {
                    tracing::error!(
                        user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        error = %persist_error,
                        "failed to persist planned workout sync failure state"
                    );
                } else if let Err(refresh_error) = self
                    .refresh
                    .refresh_range_for_user(user_id, &request.date, &request.date)
                    .await
                {
                    tracing::warn!(
                        %user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        %refresh_error,
                        "planned workout sync failure state persisted but calendar view refresh failed"
                    );
                }
                Err(error)
            }
        }
    }
}

impl<Intervals, Projections, Syncs, Hidden, Time, Refresh> CalendarUseCases
    for CalendarService<Intervals, Projections, Syncs, Hidden, Time, Refresh>
where
    Intervals: IntervalsUseCases + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Syncs: PlannedWorkoutSyncRepository + Clone,
    Hidden: HiddenCalendarEventSource + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<CalendarEvent>, CalendarError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_events_impl(&user_id, &range).await })
    }

    fn sync_planned_workout(
        &self,
        user_id: &str,
        request: SyncPlannedWorkout,
    ) -> BoxFuture<Result<CalendarEvent, CalendarError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.sync_planned_workout_impl(&user_id, request).await })
    }
}

fn build_projected_calendar_event(
    day: TrainingPlanProjectedDay,
    sync_record: Option<&PlannedWorkoutSyncRecord>,
) -> CalendarEvent {
    let payload_hash = projected_day_payload_hash(&day);
    let linked_intervals_event_id = sync_record.and_then(|record| record.intervals_event_id);
    let status = projected_day_sync_status(sync_record, &payload_hash);
    let event_id = linked_intervals_event_id
        .unwrap_or_else(|| synthetic_event_id(&day.operation_key, &day.date));
    let projected_workout_id = projected_workout_id(&day.operation_key, &day.date);

    CalendarEvent {
        calendar_entry_id: format!("predicted:{projected_workout_id}"),
        event: Event {
            id: event_id,
            start_date_local: day.date.clone(),
            event_type: Some("Ride".to_string()),
            name: projected_workout_name(&day),
            category: EventCategory::Workout,
            description: None,
            indoor: false,
            color: None,
            workout_doc: day.workout.as_ref().map(serialize_projected_workout),
        },
        source: CalendarEventSource::Predicted,
        projected_workout: Some(CalendarProjectedWorkout {
            projected_workout_id,
            operation_key: day.operation_key.clone(),
            date: day.date.clone(),
            source_workout_id: day.workout_id,
        }),
        sync_status: Some(status),
        linked_intervals_event_id,
    }
}

fn projected_day_sync_status(
    sync_record: Option<&PlannedWorkoutSyncRecord>,
    payload_hash: &str,
) -> PlannedWorkoutSyncStatus {
    match sync_record {
        None => PlannedWorkoutSyncStatus::Unsynced,
        Some(record)
            if record
                .synced_payload_hash
                .as_deref()
                .is_some_and(|hash| hash != payload_hash) =>
        {
            PlannedWorkoutSyncStatus::Modified
        }
        Some(record) => match record.status {
            PlannedWorkoutSyncStatus::Pending => PlannedWorkoutSyncStatus::Pending,
            PlannedWorkoutSyncStatus::Failed => PlannedWorkoutSyncStatus::Failed,
            PlannedWorkoutSyncStatus::Synced => PlannedWorkoutSyncStatus::Synced,
            PlannedWorkoutSyncStatus::Modified => PlannedWorkoutSyncStatus::Modified,
            PlannedWorkoutSyncStatus::Unsynced => PlannedWorkoutSyncStatus::Unsynced,
        },
    }
}

async fn find_existing_remote_event<Intervals>(
    intervals: &Intervals,
    user_id: &str,
    projected_day: &TrainingPlanProjectedDay,
    payload_hash: &str,
) -> Result<Option<Event>, CalendarError>
where
    Intervals: IntervalsUseCases,
{
    let date_range = DateRange {
        oldest: projected_day.date.clone(),
        newest: projected_day.date.clone(),
    };
    let events = intervals
        .list_events(user_id, &date_range)
        .await
        .map_err(map_intervals_error)?;

    Ok(events.into_iter().find(|event| {
        event.category == EventCategory::Workout
            && event.start_date_local.starts_with(&projected_day.date)
            && projected_event_payload_hash(
                &projected_day.date,
                event.name.as_deref(),
                event
                    .description
                    .as_deref()
                    .or(event.workout_doc.as_deref()),
            ) == payload_hash
    }))
}

fn build_create_event(day: &TrainingPlanProjectedDay) -> CreateEvent {
    CreateEvent {
        category: EventCategory::Workout,
        start_date_local: projected_event_start_date_local(&day.date),
        event_type: Some("Ride".to_string()),
        name: projected_workout_name(day),
        description: projected_workout_sync_description(day),
        indoor: false,
        color: None,
        workout_doc: None,
        file_upload: None,
    }
}

fn build_update_event(day: &TrainingPlanProjectedDay, existing_event: &Event) -> UpdateEvent {
    UpdateEvent {
        category: Some(EventCategory::Workout),
        start_date_local: Some(projected_event_start_date_local(&day.date)),
        event_type: existing_event
            .event_type
            .clone()
            .or_else(|| Some("Ride".to_string())),
        name: projected_workout_name(day),
        description: merge_event_description(
            existing_event.description.as_deref(),
            projected_workout_sync_description(day).as_deref(),
        ),
        indoor: Some(existing_event.indoor),
        color: existing_event.color.clone(),
        workout_doc: None,
        file_upload: None,
    }
}

fn projected_workout_name(day: &TrainingPlanProjectedDay) -> Option<String> {
    day.workout.as_ref().and_then(|workout| {
        workout
            .lines
            .iter()
            .find_map(|line| line.text().map(ToString::to_string))
    })
}

fn serialize_projected_workout(workout: &crate::domain::intervals::PlannedWorkout) -> String {
    crate::domain::intervals::serialize_planned_workout(workout)
}

fn projected_event_start_date_local(date: &str) -> String {
    format!("{date}T00:00:00")
}

fn projected_workout_sync_description(day: &TrainingPlanProjectedDay) -> Option<String> {
    let workout = day.workout.as_ref()?;
    let workout_name = projected_workout_name(day);

    let lines = workout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let is_title_line = index == 0
                && line
                    .text()
                    .zip(workout_name.as_deref())
                    .is_some_and(|(text, name)| text == name);

            (!is_title_line).then(|| serialize_projected_workout_line(line))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        workout_name
    } else {
        Some(lines.join("\n"))
    }
}

fn serialize_projected_workout_line(line: &crate::domain::intervals::PlannedWorkoutLine) -> String {
    match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => text.text.clone(),
        crate::domain::intervals::PlannedWorkoutLine::Repeat(repeat) => match &repeat.title {
            Some(title) => format!("{title} {}x", repeat.count),
            None => format!("{}x", repeat.count),
        },
        crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
            let duration = format_projected_step_duration(step.duration_seconds);
            let target = format_projected_step_target(&step.target);
            match step.kind {
                crate::domain::intervals::PlannedWorkoutStepKind::Steady => {
                    format!("- {duration} {target}")
                }
                crate::domain::intervals::PlannedWorkoutStepKind::Ramp => {
                    format!("- {duration} ramp {target}")
                }
            }
        }
    }
}

fn format_projected_step_duration(duration_seconds: i32) -> String {
    if duration_seconds % 60 == 0 {
        format!("{}m", duration_seconds / 60)
    } else {
        format!("{duration_seconds}s")
    }
}

fn format_projected_step_target(target: &crate::domain::intervals::PlannedWorkoutTarget) -> String {
    match target {
        crate::domain::intervals::PlannedWorkoutTarget::PercentFtp { min, max } => {
            if (min - max).abs() < f64::EPSILON {
                format!("{}%", trim_decimal(*min))
            } else {
                format!("{}-{}%", trim_decimal(*min), trim_decimal(*max))
            }
        }
        crate::domain::intervals::PlannedWorkoutTarget::WattsRange { min, max } => {
            if min == max {
                format!("{min}W")
            } else {
                format!("{min}-{max}W")
            }
        }
    }
}

fn trim_decimal(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

fn merge_event_description(existing: Option<&str>, projected: Option<&str>) -> Option<String> {
    match (
        existing.map(str::trim).filter(|value| !value.is_empty()),
        projected,
    ) {
        (None, None) => None,
        (Some(existing), None) => Some(existing.to_string()),
        (None, Some(projected)) => Some(projected.to_string()),
        (Some(existing), Some(projected)) if existing.contains(projected) => {
            Some(existing.to_string())
        }
        (Some(existing), Some(projected)) => Some(format!("{existing}\n\n{projected}")),
    }
}

fn projected_day_payload_hash(day: &TrainingPlanProjectedDay) -> String {
    projected_event_payload_hash(
        &day.date,
        projected_workout_name(day).as_deref(),
        projected_workout_sync_description(day).as_deref(),
    )
}

fn projected_event_payload_hash(
    date: &str,
    name: Option<&str>,
    workout_doc: Option<&str>,
) -> String {
    let digest = Sha256::digest(format!(
        "{date}\n{}\n{}",
        name.unwrap_or_default(),
        workout_doc.unwrap_or_default()
    ));
    format!("{digest:x}")
}

fn projected_workout_id(operation_key: &str, date: &str) -> String {
    format!("{operation_key}:{date}")
}

fn synthetic_event_id(operation_key: &str, date: &str) -> i64 {
    const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let digest = Sha256::digest(format!("{operation_key}:{date}"));
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    ((value % MAX_JS_SAFE_INTEGER) + 1) as i64
}

fn is_date_in_range(date: &str, range: &DateRange) -> bool {
    date >= range.oldest.as_str() && date <= range.newest.as_str()
}

fn map_intervals_error(error: IntervalsError) -> CalendarError {
    match error {
        IntervalsError::NotFound => CalendarError::NotFound,
        IntervalsError::Unauthenticated => CalendarError::Unauthenticated,
        IntervalsError::CredentialsNotConfigured => CalendarError::CredentialsNotConfigured,
        IntervalsError::ApiError(message)
        | IntervalsError::ConnectionError(message)
        | IntervalsError::Internal(message) => CalendarError::Unavailable(message),
    }
}

fn map_training_plan_error(
    error: crate::domain::training_plan::TrainingPlanError,
) -> CalendarError {
    match error {
        crate::domain::training_plan::TrainingPlanError::Validation(message) => {
            CalendarError::Validation(message)
        }
        crate::domain::training_plan::TrainingPlanError::Unavailable(message) => {
            CalendarError::Unavailable(message)
        }
        crate::domain::training_plan::TrainingPlanError::Repository(message) => {
            CalendarError::Internal(message)
        }
    }
}
