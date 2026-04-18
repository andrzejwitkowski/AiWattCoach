use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Duration, NaiveDate};

use crate::domain::{
    intervals::{
        find_best_activity_match, parse_workout_doc, Activity, Event, EventCategory,
        PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutTarget, WorkoutSegment,
    },
    training_context::model::{
        FuturePlannedEventContext, HistoricalTrainingContext, HistoricalWorkoutContext,
        PlannedWorkoutBlockContext, PlannedWorkoutContext, PlannedWorkoutReference,
        RecentDayContext, RecentWorkoutContext, SpecialDayContext, UpcomingDayContext,
    },
    training_load::{FtpHistoryEntry, TrainingLoadDailySnapshot},
};

use super::{
    dates::{date_key, date_range_inclusive, round_to_2},
    history::{
        average_metric, average_recent_tss, build_daily_tss_map, build_load_trend, ewma_latest,
        recent_slice,
    },
    power::{compress_power_stream, extract_and_average_stream, extract_power_stream},
};

pub(super) fn build_historical_context(
    start: NaiveDate,
    end: NaiveDate,
    activities: &[Activity],
    load_sources: HistoricalLoadSources<'_>,
    workout_sources: HistoricalWorkoutSources<'_>,
    configured_ftp: Option<i32>,
) -> HistoricalTrainingContext {
    let complete_snapshot_coverage =
        has_complete_snapshot_coverage(load_sources.daily_snapshots, start, end);
    let ftp_history_in_window = ftp_history_through_date(load_sources.ftp_history, end);
    let workouts = activities
        .iter()
        .map(|activity| {
            let compressed_power_levels = workout_sources
                .detailed_activities_by_id
                .get(&activity.id)
                .map(|detailed| {
                    compress_power_stream(
                        &extract_power_stream(&detailed.details.streams),
                        detailed.metrics.ftp_watts.or(configured_ftp),
                    )
                })
                .unwrap_or_default();

            HistoricalWorkoutContext {
                date: date_key(&activity.start_date_local),
                activity_id: activity.id.clone(),
                name: activity.name.clone(),
                activity_type: activity.activity_type.clone(),
                duration_seconds: activity
                    .elapsed_time_seconds
                    .or(activity.moving_time_seconds),
                training_stress_score: activity.metrics.training_stress_score,
                intensity_factor: activity.metrics.intensity_factor,
                efficiency_factor: activity.metrics.efficiency_factor,
                normalized_power_watts: activity.metrics.normalized_power_watts,
                ftp_watts: activity.metrics.ftp_watts,
                workout_recap: workout_sources
                    .workout_recaps_by_id
                    .get(&activity.id)
                    .cloned(),
                variability_index: activity.metrics.variability_index,
                compressed_power_levels,
                interval_blocks: workout_sources
                    .recent_interval_blocks_by_activity_id
                    .get(&activity.id)
                    .cloned()
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let total_tss = if !complete_snapshot_coverage {
        activities
            .iter()
            .filter_map(|activity| activity.metrics.training_stress_score)
            .sum::<i32>()
    } else {
        load_sources
            .daily_snapshots
            .iter()
            .filter_map(|snapshot| snapshot.daily_tss)
            .sum::<i32>()
    };
    let daily_tss = build_daily_tss_map(start, end, activities);
    let load_trend = if !complete_snapshot_coverage {
        build_load_trend(&daily_tss, 42, 42)
    } else {
        build_load_trend_from_snapshots(load_sources.daily_snapshots, 42)
    };
    let latest_snapshot = complete_snapshot_coverage
        .then(|| load_sources.daily_snapshots.last())
        .flatten();
    let ctl = latest_snapshot
        .and_then(|snapshot| snapshot.ctl)
        .or_else(|| ewma_latest(&daily_tss, 42));
    let atl = latest_snapshot
        .and_then(|snapshot| snapshot.atl)
        .or_else(|| ewma_latest(&daily_tss, 7));
    let tsb = latest_snapshot
        .and_then(|snapshot| snapshot.tsb)
        .or_else(|| match (ctl, atl) {
            (Some(ctl), Some(atl)) => Some(round_to_2(ctl - atl)),
            _ => None,
        });
    let latest_history_entry = ftp_history_in_window
        .iter()
        .max_by_key(|entry| entry.effective_from_date.as_str());
    let ftp_current = latest_history_entry
        .and_then(|entry| (entry.ftp_watts > 0).then_some(entry.ftp_watts))
        .or_else(|| latest_snapshot.and_then(|snapshot| snapshot.ftp_effective_watts))
        .or_else(|| {
            activities
                .iter()
                .filter_map(|activity| {
                    activity
                        .metrics
                        .ftp_watts
                        .map(|ftp| (date_key(&activity.start_date_local), ftp))
                })
                .max_by_key(|(date, _)| date.clone())
                .map(|(_, ftp)| ftp)
        });
    let ftp_change = ftp_change_from_history(&ftp_history_in_window).or_else(|| {
        let ftp_samples = activities
            .iter()
            .filter_map(|activity| {
                activity
                    .metrics
                    .ftp_watts
                    .map(|ftp| (date_key(&activity.start_date_local), ftp))
            })
            .collect::<Vec<_>>();
        let ftp_earliest = ftp_samples
            .iter()
            .min_by_key(|(date, _)| date.as_str())
            .map(|(_, ftp)| *ftp);
        ftp_current
            .zip(ftp_earliest)
            .map(|(latest, earliest)| latest - earliest)
    });
    let average_tss_7d = latest_snapshot
        .and_then(|snapshot| snapshot.rolling_tss_7d)
        .or_else(|| average_recent_tss(&daily_tss, 7));
    let average_tss_28d = latest_snapshot
        .and_then(|snapshot| snapshot.rolling_tss_28d)
        .or_else(|| average_recent_tss(&daily_tss, 28));
    let recent_28 = recent_slice(activities, end - Duration::days(27));
    let average_if_28d = latest_snapshot
        .and_then(|snapshot| snapshot.average_if_28d)
        .or_else(|| {
            average_metric(
                recent_28
                    .iter()
                    .filter_map(|activity| activity.metrics.intensity_factor),
            )
        });
    let average_ef_28d = latest_snapshot
        .and_then(|snapshot| snapshot.average_ef_28d)
        .or_else(|| {
            average_metric(
                recent_28
                    .iter()
                    .filter_map(|activity| activity.metrics.efficiency_factor),
            )
        });

    HistoricalTrainingContext {
        window_start: start.format("%Y-%m-%d").to_string(),
        window_end: end.format("%Y-%m-%d").to_string(),
        activity_count: activities.len(),
        total_tss,
        ctl,
        atl,
        tsb,
        ftp_current,
        ftp_change,
        average_tss_7d,
        average_tss_28d,
        average_if_28d,
        average_ef_28d,
        load_trend,
        workouts,
    }
}

pub(super) struct HistoricalLoadSources<'a> {
    pub(super) daily_snapshots: &'a [TrainingLoadDailySnapshot],
    pub(super) ftp_history: &'a [FtpHistoryEntry],
}

pub(super) struct HistoricalWorkoutSources<'a> {
    pub(super) detailed_activities_by_id: &'a HashMap<String, Activity>,
    pub(super) workout_recaps_by_id: &'a HashMap<String, String>,
    pub(super) recent_interval_blocks_by_activity_id:
        &'a HashMap<String, Vec<PlannedWorkoutBlockContext>>,
}

fn build_load_trend_from_snapshots(
    daily_snapshots: &[TrainingLoadDailySnapshot],
    trend_days: usize,
) -> Vec<crate::domain::training_context::model::HistoricalLoadTrendPoint> {
    let start_index = daily_snapshots.len().saturating_sub(trend_days);
    daily_snapshots[start_index..]
        .iter()
        .map(
            |snapshot| crate::domain::training_context::model::HistoricalLoadTrendPoint {
                date: snapshot.date.clone(),
                sample_days: 1,
                period_tss: snapshot.daily_tss.unwrap_or_default(),
                rolling_tss_7d: snapshot.rolling_tss_7d,
                rolling_tss_28d: snapshot.rolling_tss_28d,
                ctl: snapshot.ctl,
                atl: snapshot.atl,
                tsb: snapshot.tsb,
            },
        )
        .collect()
}

fn ftp_change_from_history(ftp_history: &[FtpHistoryEntry]) -> Option<i32> {
    let earliest = ftp_history
        .iter()
        .filter(|entry| entry.ftp_watts > 0)
        .min_by_key(|entry| entry.effective_from_date.as_str())?;
    let latest = ftp_history
        .iter()
        .max_by_key(|entry| entry.effective_from_date.as_str())?;

    if latest.ftp_watts <= 0 {
        return None;
    }

    Some(latest.ftp_watts - earliest.ftp_watts)
}

fn ftp_history_through_date(
    ftp_history: &[FtpHistoryEntry],
    end: NaiveDate,
) -> Vec<FtpHistoryEntry> {
    let end = end.format("%Y-%m-%d").to_string();
    ftp_history
        .iter()
        .filter(|entry| entry.effective_from_date <= end)
        .cloned()
        .collect()
}

fn has_complete_snapshot_coverage(
    daily_snapshots: &[TrainingLoadDailySnapshot],
    start: NaiveDate,
    end: NaiveDate,
) -> bool {
    let expected_days = (end - start).num_days().max(0) + 1;
    if daily_snapshots.len() as i64 != expected_days {
        return false;
    }

    for (offset, snapshot) in daily_snapshots.iter().enumerate() {
        let expected_date = start + Duration::days(offset as i64);
        if snapshot.date != expected_date.format("%Y-%m-%d").to_string() {
            return false;
        }
    }

    true
}

pub(super) fn build_recent_day_contexts(
    start: NaiveDate,
    end: NaiveDate,
    events: &[Event],
    detailed_activities: &[Activity],
    summary_lookup: RecentWorkoutSummaryLookup<'_>,
    matched: &EventActivityMatches,
    configured_ftp: Option<i32>,
) -> Vec<RecentDayContext> {
    let activities_by_date = group_activities_by_date(detailed_activities);
    let events_by_date = group_events_by_date(events);

    date_range_inclusive(start, end)
        .into_iter()
        .map(|date| {
            let date_key = date.format("%Y-%m-%d").to_string();
            let day_activities = activities_by_date
                .get(&date_key)
                .cloned()
                .unwrap_or_default();
            let day_events = events_by_date.get(&date_key).cloned().unwrap_or_default();
            let planned_workouts = day_events
                .iter()
                .filter(|event| event.category == EventCategory::Workout)
                .filter(|event| !matched.event_to_activity.contains_key(&event.id))
                .map(|event| build_planned_workout(event, false))
                .collect::<Vec<_>>();
            let special_days = day_events
                .iter()
                .filter(|event| is_special_event(event))
                .map(build_special_day)
                .collect::<Vec<_>>();
            let sick_note = infer_sick_note(&special_days);
            let workouts = day_activities
                .iter()
                .map(|activity| {
                    build_recent_workout(
                        activity,
                        matched.activity_to_event.get(&activity.id),
                        summary_lookup.rpe_by_workout_id,
                        summary_lookup.recap_by_workout_id,
                        configured_ftp,
                    )
                })
                .collect::<Vec<_>>();

            RecentDayContext {
                date: date_key,
                free_day: workouts.is_empty()
                    && planned_workouts.is_empty()
                    && special_days.is_empty(),
                sick_day: sick_note.is_some(),
                sick_note,
                workouts,
                planned_workouts,
                special_days,
            }
        })
        .collect()
}

pub(super) fn build_upcoming_day_contexts(
    start: NaiveDate,
    end: NaiveDate,
    events: &[Event],
) -> Vec<UpcomingDayContext> {
    let events_by_date = group_events_by_date(events);

    date_range_inclusive(start, end)
        .into_iter()
        .map(|date| {
            let date_key = date.format("%Y-%m-%d").to_string();
            let day_events = events_by_date.get(&date_key).cloned().unwrap_or_default();
            let planned_workouts = day_events
                .iter()
                .filter(|event| event.category == EventCategory::Workout)
                .map(|event| build_planned_workout(event, false))
                .collect::<Vec<_>>();
            let special_days = day_events
                .iter()
                .filter(|event| is_special_event(event))
                .map(build_special_day)
                .collect::<Vec<_>>();

            UpcomingDayContext {
                date: date_key,
                free_day: planned_workouts.is_empty() && special_days.is_empty(),
                planned_workouts,
                special_days,
            }
        })
        .collect()
}

pub(super) fn build_future_planned_event_contexts(
    events: &[Event],
    configured_ftp: Option<i32>,
) -> Vec<FuturePlannedEventContext> {
    let mut future_events = events
        .iter()
        .map(|event| {
            let parsed = parse_workout_doc(event.structured_workout_text(), configured_ftp);
            let duration_seconds = (parsed.summary.total_duration_seconds > 0)
                .then_some(parsed.summary.total_duration_seconds);

            FuturePlannedEventContext {
                event_id: event.id,
                start_date_local: event.start_date_local.clone(),
                category: event.category.as_str().to_string(),
                event_type: event.event_type.clone(),
                name: event.name.clone(),
                description: event.description.clone(),
                estimated_duration_seconds: duration_seconds,
                estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
                estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
                estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
            }
        })
        .collect::<Vec<_>>();

    future_events.sort_by(|left, right| {
        left.start_date_local
            .cmp(&right.start_date_local)
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.name.cmp(&right.name))
    });

    future_events
}

fn build_recent_workout(
    activity: &Activity,
    matched_event: Option<&Event>,
    summaries_by_id: &HashMap<String, u8>,
    workout_recaps_by_id: &HashMap<String, String>,
    configured_ftp: Option<i32>,
) -> RecentWorkoutContext {
    let resolved_ftp = activity.metrics.ftp_watts.or(configured_ftp);
    let compressed_power_levels = compress_power_stream(
        &extract_power_stream(&activity.details.streams),
        resolved_ftp,
    );
    let cadence_values_5s = extract_and_average_stream(&activity.details.streams, "cadence");
    let planned_workout = matched_event.map(|event| {
        let parsed = parse_workout_doc(event.structured_workout_text(), resolved_ftp);

        PlannedWorkoutReference {
            event_id: event.id,
            start_date_local: event.start_date_local.clone(),
            name: event.name.clone(),
            category: event.category.as_str().to_string(),
            interval_blocks: build_planned_workout_blocks(&parsed.segments, resolved_ftp),
            raw_workout_doc: event.workout_doc.clone(),
            estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
            estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
            estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
            completed: true,
        }
    });
    let summary_lookup_id = matched_event
        .map(|event| event.id.to_string())
        .filter(|event_id| {
            !summaries_by_id.contains_key(&activity.id) && summaries_by_id.contains_key(event_id)
        });
    let recap_lookup_id = matched_event
        .map(|event| event.id.to_string())
        .filter(|event_id| {
            !workout_recaps_by_id.contains_key(&activity.id)
                && workout_recaps_by_id.contains_key(event_id)
        });

    RecentWorkoutContext {
        activity_id: activity.id.clone(),
        start_date_local: activity.start_date_local.clone(),
        name: activity.name.clone(),
        activity_type: activity.activity_type.clone(),
        training_stress_score: activity.metrics.training_stress_score,
        efficiency_factor: activity.metrics.efficiency_factor,
        intensity_factor: activity.metrics.intensity_factor,
        normalized_power_watts: activity.metrics.normalized_power_watts,
        ftp_watts: activity.metrics.ftp_watts,
        rpe: summaries_by_id.get(&activity.id).copied().or_else(|| {
            summary_lookup_id
                .as_ref()
                .and_then(|id| summaries_by_id.get(id).copied())
        }),
        workout_recap: workout_recaps_by_id.get(&activity.id).cloned().or_else(|| {
            recap_lookup_id
                .as_ref()
                .and_then(|id| workout_recaps_by_id.get(id).cloned())
        }),
        variability_index: activity.metrics.variability_index,
        compressed_power_levels,
        cadence_values_5s,
        planned_workout,
    }
}

pub(super) fn projected_workout_name(workout: &PlannedWorkout) -> Option<String> {
    workout
        .lines
        .iter()
        .find_map(|line| line.text().map(ToString::to_string))
}

pub(super) fn projected_interval_blocks(
    workout: &PlannedWorkout,
) -> Vec<PlannedWorkoutBlockContext> {
    let mut blocks = Vec::new();
    let mut repeated_steps = Vec::new();
    let mut repeat_count: Option<usize> = None;

    for line in &workout.lines {
        match line {
            PlannedWorkoutLine::Repeat(repeat) => {
                flush_repeated_steps(&mut blocks, &mut repeated_steps, repeat_count.take());
                repeat_count = Some(repeat.count);
            }
            PlannedWorkoutLine::Step(step) => {
                let block = planned_block_from_step(step);
                if repeat_count.is_some() {
                    repeated_steps.push(block);
                } else {
                    blocks.push(block);
                }
            }
            PlannedWorkoutLine::Text(_) => {
                flush_repeated_steps(&mut blocks, &mut repeated_steps, repeat_count.take());
            }
        }
    }

    flush_repeated_steps(&mut blocks, &mut repeated_steps, repeat_count.take());
    blocks
}

fn planned_block_from_step(
    step: &crate::domain::intervals::PlannedWorkoutStep,
) -> PlannedWorkoutBlockContext {
    let (min_percent_ftp, max_percent_ftp, min_target_watts, max_target_watts) = match &step.target
    {
        PlannedWorkoutTarget::PercentFtp { min, max } => (Some(*min), Some(*max), None, None),
        PlannedWorkoutTarget::WattsRange { min, max } => (None, None, Some(*min), Some(*max)),
    };

    PlannedWorkoutBlockContext {
        duration_seconds: step.duration_seconds,
        min_percent_ftp,
        max_percent_ftp,
        min_target_watts,
        max_target_watts,
    }
}

fn flush_repeated_steps(
    blocks: &mut Vec<PlannedWorkoutBlockContext>,
    repeated_steps: &mut Vec<PlannedWorkoutBlockContext>,
    repeat_count: Option<usize>,
) {
    if repeated_steps.is_empty() {
        return;
    }

    let count = repeat_count.unwrap_or(1);
    for _ in 0..count {
        blocks.extend(repeated_steps.iter().cloned());
    }
    repeated_steps.clear();
}

fn build_projected_planned_workout(
    event: &Event,
    completed: bool,
    ftp_watts: Option<i32>,
) -> PlannedWorkoutContext {
    let parsed = parse_workout_doc(event.structured_workout_text(), ftp_watts);
    PlannedWorkoutContext {
        event_id: event.id,
        start_date_local: event.start_date_local.clone(),
        name: event.name.clone(),
        category: event.category.as_str().to_string(),
        interval_blocks: build_planned_workout_blocks(&parsed.segments, ftp_watts),
        raw_workout_doc: event.workout_doc.clone(),
        estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
        estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
        estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
        completed,
    }
}

fn build_planned_workout(event: &Event, completed: bool) -> PlannedWorkoutContext {
    build_projected_planned_workout(event, completed, None)
}

fn build_special_day(event: &Event) -> SpecialDayContext {
    SpecialDayContext {
        event_id: event.id,
        start_date_local: event.start_date_local.clone(),
        name: event.name.clone(),
        category: event.category.as_str().to_string(),
        description: event.description.clone(),
    }
}

fn build_planned_workout_blocks(
    segments: &[WorkoutSegment],
    ftp_watts: Option<i32>,
) -> Vec<PlannedWorkoutBlockContext> {
    segments
        .iter()
        .map(|segment| PlannedWorkoutBlockContext {
            duration_seconds: segment.duration_seconds,
            min_percent_ftp: segment.min_target_percent_ftp,
            max_percent_ftp: segment.max_target_percent_ftp,
            min_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .min_target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            }),
            max_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .max_target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            }),
        })
        .collect()
}

fn infer_sick_note(special_days: &[SpecialDayContext]) -> Option<String> {
    special_days.iter().find_map(|special| {
        let mut text = String::new();
        if let Some(name) = &special.name {
            text.push_str(name);
            text.push(' ');
        }
        if let Some(description) = &special.description {
            text.push_str(description);
        }

        let normalized = text.trim().to_ascii_lowercase();
        let tokens = normalized
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        if tokens
            .iter()
            .any(|token| matches!(*token, "sick" | "ill" | "unwell" | "fever" | "cold" | "flu"))
        {
            Some(text.trim().to_string())
        } else {
            None
        }
    })
}

#[derive(Default)]
pub(super) struct EventActivityMatches {
    pub(super) event_to_activity: HashMap<i64, String>,
    pub(super) activity_to_event: HashMap<String, Event>,
}

pub(super) struct RecentWorkoutSummaryLookup<'a> {
    pub(super) rpe_by_workout_id: &'a HashMap<String, u8>,
    pub(super) recap_by_workout_id: &'a HashMap<String, String>,
}

pub(super) fn build_event_activity_matches(
    events: &[Event],
    activities: &[Activity],
    direct_matches: &HashMap<String, Event>,
    configured_ftp: Option<i32>,
) -> EventActivityMatches {
    let mut scored_matches = Vec::new();
    let mut matches = EventActivityMatches::default();
    let mut used_event_ids = BTreeSet::new();
    let mut used_activity_ids = BTreeSet::new();

    for activity in activities {
        let Some(event) = direct_matches.get(&activity.id) else {
            continue;
        };

        used_event_ids.insert(event.id);
        used_activity_ids.insert(activity.id.clone());
        matches
            .event_to_activity
            .insert(event.id, activity.id.clone());
        matches
            .activity_to_event
            .insert(activity.id.clone(), event.clone());
    }

    for event in events
        .iter()
        .filter(|event| event.category == EventCategory::Workout)
    {
        if used_event_ids.contains(&event.id) {
            continue;
        }

        let event_date = date_key(&event.start_date_local);
        let same_day = activities
            .iter()
            .filter(|activity| {
                !used_activity_ids.contains(&activity.id)
                    && date_key(&activity.start_date_local) == event_date
            })
            .collect::<Vec<_>>();

        for activity in same_day {
            let effective_ftp = activity.metrics.ftp_watts.or(configured_ftp);
            let parsed = parse_workout_doc(event.structured_workout_text(), effective_ftp);
            if let Some(candidate) =
                find_best_activity_match(&parsed, std::slice::from_ref(activity), effective_ftp)
            {
                scored_matches.push((candidate.compliance_score, event.clone(), activity.clone()));
            }
        }
    }

    scored_matches.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (_score, event, activity) in scored_matches {
        if used_event_ids.contains(&event.id) || used_activity_ids.contains(&activity.id) {
            continue;
        }
        used_event_ids.insert(event.id);
        used_activity_ids.insert(activity.id.clone());
        matches
            .event_to_activity
            .insert(event.id, activity.id.clone());
        matches.activity_to_event.insert(activity.id, event);
    }

    matches
}

pub(super) fn projected_raw_workout_doc(workout: &PlannedWorkout) -> String {
    crate::domain::intervals::serialize_planned_workout(workout)
}

fn group_activities_by_date(activities: &[Activity]) -> BTreeMap<String, Vec<Activity>> {
    let mut grouped = BTreeMap::new();
    for activity in activities {
        grouped
            .entry(date_key(&activity.start_date_local))
            .or_insert_with(Vec::new)
            .push(activity.clone());
    }
    grouped
}

fn group_events_by_date(events: &[Event]) -> BTreeMap<String, Vec<Event>> {
    let mut grouped = BTreeMap::new();
    for event in events {
        grouped
            .entry(date_key(&event.start_date_local))
            .or_insert_with(Vec::new)
            .push(event.clone());
    }
    grouped
}

pub(super) fn infer_focus_kind(
    workout_id: &str,
    recent_days: &[RecentDayContext],
    upcoming_days: &[UpcomingDayContext],
) -> String {
    if recent_days
        .iter()
        .flat_map(|day| &day.workouts)
        .any(|workout| workout.activity_id == workout_id)
    {
        return "activity".to_string();
    }
    if recent_days
        .iter()
        .flat_map(|day| &day.workouts)
        .filter_map(|workout| workout.planned_workout.as_ref())
        .any(|planned| planned.event_id.to_string() == workout_id)
    {
        return "event".to_string();
    }
    if recent_days
        .iter()
        .flat_map(|day| &day.planned_workouts)
        .any(|planned| planned.event_id.to_string() == workout_id)
        || upcoming_days
            .iter()
            .flat_map(|day| &day.planned_workouts)
            .any(|planned| planned.event_id.to_string() == workout_id)
    {
        return "event".to_string();
    }
    "summary".to_string()
}

fn is_special_event(event: &Event) -> bool {
    !matches!(event.category, EventCategory::Workout)
}
