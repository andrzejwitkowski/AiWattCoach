use std::collections::HashMap;

use crate::domain::{
    calendar::{
        CalendarError, CalendarEvent, CalendarEventCategory, CalendarEventSource,
        CalendarProjectedWorkout, PlannedWorkoutSyncStatus,
    },
    calendar_view::{CalendarEntryKind, CalendarEntryView},
    completed_workouts::{CompletedWorkout, CompletedWorkoutSeries, CompletedWorkoutStream},
    intervals::{ActualWorkoutMatch, DateRange, MatchedWorkoutInterval},
};

use super::{
    errors::{map_calendar_entry_view_error, map_completed_workout_error},
    projected::synthetic_event_id,
    CalendarService,
};

impl<Intervals, Entries, Projections, Syncs, Time, Tokens, PollStates, Refresh, Completed>
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        Refresh,
        Completed,
    >
where
    Intervals: crate::domain::intervals::IntervalsUseCases + Clone,
    Entries: crate::domain::calendar_view::CalendarEntryViewRepository + Clone,
    Completed: crate::domain::completed_workouts::CompletedWorkoutRepository + Clone,
    Projections: crate::domain::training_plan::TrainingPlanProjectionRepository + Clone,
    Syncs: crate::domain::calendar::PlannedWorkoutSyncRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Tokens: crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository + Clone,
    PollStates: crate::domain::external_sync::ProviderPollStateRepository + Clone,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort + Clone,
{
    pub(super) async fn list_events_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<CalendarEvent>, CalendarError> {
        let entries = self
            .entries
            .list_by_user_id_and_date_range(user_id, &range.oldest, &range.newest)
            .await
            .map_err(map_calendar_entry_view_error)?;
        let completed_by_id = self
            .completed_workouts
            .list_by_user_id_and_date_range(user_id, &range.oldest, &range.newest)
            .await
            .map_err(map_completed_workout_error)?
            .into_iter()
            .map(|workout| (workout.completed_workout_id.clone(), workout))
            .collect::<HashMap<_, _>>();
        let mut events = entries
            .into_iter()
            .filter_map(|entry| map_calendar_entry_to_event(entry, &completed_by_id))
            .collect::<Vec<_>>();

        events.sort_by(|left, right| {
            left.start_date_local
                .cmp(&right.start_date_local)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(events)
    }
}

fn map_calendar_entry_to_event(
    entry: CalendarEntryView,
    completed_by_id: &HashMap<String, CompletedWorkout>,
) -> Option<CalendarEvent> {
    let category = match entry.entry_kind {
        CalendarEntryKind::CompletedWorkout => return None,
        CalendarEntryKind::PlannedWorkout => CalendarEventCategory::Workout,
        CalendarEntryKind::Race => CalendarEventCategory::Race,
        CalendarEntryKind::SpecialDay => CalendarEventCategory::Note,
    };
    let id = entry
        .sync
        .as_ref()
        .and_then(|sync| sync.linked_intervals_event_id)
        .unwrap_or_else(|| synthetic_event_id(&entry.entry_id, &entry.date));
    let name = Some(entry.title.clone()).filter(|value| !value.trim().is_empty());
    let start_date_local = match entry.entry_kind {
        CalendarEntryKind::CompletedWorkout => entry
            .start_date_local
            .clone()
            .unwrap_or_else(|| entry.date.clone()),
        CalendarEntryKind::PlannedWorkout
        | CalendarEntryKind::Race
        | CalendarEntryKind::SpecialDay => entry.date.clone(),
    };
    let projected_workout = match entry.entry_kind {
        CalendarEntryKind::PlannedWorkout => {
            entry
                .planned_workout_id
                .as_ref()
                .and_then(|planned_workout_id| {
                    parse_projected_workout(
                        planned_workout_id,
                        &entry.date,
                        entry.rest_day,
                        entry.rest_day_reason.as_deref(),
                    )
                })
        }
        _ => None,
    };
    let source = if projected_workout.is_some() {
        CalendarEventSource::Predicted
    } else {
        CalendarEventSource::Intervals
    };
    let sync_status = match entry.entry_kind {
        CalendarEntryKind::PlannedWorkout => entry
            .sync
            .as_ref()
            .and_then(|sync| map_calendar_sync_status(sync.sync_status.as_deref())),
        _ => None,
    };
    let raw_workout_doc = match entry.entry_kind {
        CalendarEntryKind::PlannedWorkout => entry.raw_workout_doc.clone(),
        _ => None,
    };
    let actual_workout = entry
        .completed_workout_id
        .as_deref()
        .and_then(|completed_workout_id| completed_by_id.get(completed_workout_id))
        .map(map_completed_workout_to_actual_workout_match);

    Some(CalendarEvent {
        id,
        calendar_entry_id: entry.entry_id,
        start_date_local,
        name,
        category,
        description: entry.description,
        rest_day: entry.rest_day,
        rest_day_reason: entry.rest_day_reason,
        indoor: false,
        color: None,
        raw_workout_doc,
        source,
        projected_workout,
        sync_status,
        linked_intervals_event_id: entry.sync.and_then(|sync| sync.linked_intervals_event_id),
        actual_workout,
    })
}

fn map_calendar_sync_status(value: Option<&str>) -> Option<PlannedWorkoutSyncStatus> {
    match value? {
        "unsynced" => Some(PlannedWorkoutSyncStatus::Unsynced),
        "pending" => Some(PlannedWorkoutSyncStatus::Pending),
        "synced" => Some(PlannedWorkoutSyncStatus::Synced),
        "modified" => Some(PlannedWorkoutSyncStatus::Modified),
        "failed" => Some(PlannedWorkoutSyncStatus::Failed),
        _ => None,
    }
}

fn map_completed_workout_to_actual_workout_match(workout: &CompletedWorkout) -> ActualWorkoutMatch {
    ActualWorkoutMatch {
        activity_id: workout
            .source_activity_id
            .clone()
            .unwrap_or_else(|| legacy_activity_id(&workout.completed_workout_id).to_string()),
        activity_name: workout.name.clone(),
        start_date_local: workout.start_date_local.clone(),
        power_values: integer_stream_values(&workout.details.streams, &["watts"]),
        cadence_values: integer_stream_values(&workout.details.streams, &["cadence"]),
        heart_rate_values: integer_stream_values(
            &workout.details.streams,
            &["heartrate", "heart_rate"],
        ),
        speed_values: float_stream_values(&workout.details.streams, &["velocity_smooth", "speed"]),
        average_power_watts: workout.metrics.average_power_watts,
        normalized_power_watts: workout.metrics.normalized_power_watts,
        training_stress_score: workout.metrics.training_stress_score,
        intensity_factor: workout.metrics.intensity_factor,
        compliance_score: 1.0,
        matched_intervals: workout
            .details
            .intervals
            .iter()
            .enumerate()
            .map(|(index, interval)| MatchedWorkoutInterval {
                planned_segment_order: index,
                planned_label: interval
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("Interval {}", index + 1)),
                planned_duration_seconds: interval
                    .elapsed_time_seconds
                    .or(interval.moving_time_seconds)
                    .unwrap_or(0),
                target_percent_ftp: None,
                min_target_percent_ftp: None,
                max_target_percent_ftp: None,
                zone_id: interval.zone,
                actual_interval_id: interval.id,
                actual_start_time_seconds: interval.start_time_seconds,
                actual_end_time_seconds: interval.end_time_seconds,
                average_power_watts: interval.average_power_watts,
                normalized_power_watts: interval.normalized_power_watts,
                average_heart_rate_bpm: interval.average_heart_rate_bpm,
                average_cadence_rpm: interval.average_cadence_rpm,
                average_speed_mps: interval.average_speed_mps,
                compliance_score: 1.0,
            })
            .collect(),
    }
}

fn integer_stream_values(streams: &[CompletedWorkoutStream], stream_types: &[&str]) -> Vec<i32> {
    streams
        .iter()
        .find(|stream| {
            stream_types
                .iter()
                .any(|stream_type| stream.stream_type.eq_ignore_ascii_case(stream_type))
        })
        .and_then(|stream| stream.primary_series.as_ref())
        .map(integer_series_values)
        .unwrap_or_default()
}

fn integer_series_values(series: &CompletedWorkoutSeries) -> Vec<i32> {
    match series {
        CompletedWorkoutSeries::Integers(values) => values
            .iter()
            .filter_map(|value| i32::try_from(*value).ok())
            .collect(),
        CompletedWorkoutSeries::Floats(values) => values
            .iter()
            .filter(|value| value.is_finite())
            .map(|value| value.round() as i32)
            .collect(),
        CompletedWorkoutSeries::Bools(_) | CompletedWorkoutSeries::Strings(_) => Vec::new(),
    }
}

fn float_stream_values(streams: &[CompletedWorkoutStream], stream_types: &[&str]) -> Vec<f64> {
    streams
        .iter()
        .find(|stream| {
            stream_types
                .iter()
                .any(|stream_type| stream.stream_type.eq_ignore_ascii_case(stream_type))
        })
        .and_then(|stream| stream.primary_series.as_ref())
        .map(float_series_values)
        .unwrap_or_default()
}

fn float_series_values(series: &CompletedWorkoutSeries) -> Vec<f64> {
    match series {
        CompletedWorkoutSeries::Integers(values) => {
            values.iter().map(|value| *value as f64).collect()
        }
        CompletedWorkoutSeries::Floats(values) => values.clone(),
        CompletedWorkoutSeries::Bools(_) | CompletedWorkoutSeries::Strings(_) => Vec::new(),
    }
}

fn legacy_activity_id(completed_workout_id: &str) -> &str {
    completed_workout_id
        .strip_prefix("intervals-activity:")
        .unwrap_or(completed_workout_id)
}

fn parse_projected_workout(
    planned_workout_id: &str,
    date: &str,
    rest_day: bool,
    rest_day_reason: Option<&str>,
) -> Option<CalendarProjectedWorkout> {
    let (operation_key, projected_date) = planned_workout_id.rsplit_once(':')?;
    if projected_date != date {
        return None;
    }

    Some(CalendarProjectedWorkout {
        projected_workout_id: planned_workout_id.to_string(),
        operation_key: operation_key.to_string(),
        date: projected_date.to_string(),
        source_workout_id: planned_workout_id.to_string(),
        rest_day,
        rest_day_reason: rest_day_reason.map(ToString::to_string),
    })
}
