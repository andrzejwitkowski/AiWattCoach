use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;

use crate::domain::{
    intervals::{parse_workout_doc, Activity},
    training_context::model::{HistoricalLoadTrendPoint, PlannedWorkoutBlockContext},
};

use super::{
    context::EventActivityMatches,
    dates::{activity_date, date_range_inclusive, round_to_2},
};

pub(super) fn build_daily_tss_map(
    start: NaiveDate,
    end: NaiveDate,
    activities: &[Activity],
) -> BTreeMap<NaiveDate, i32> {
    let mut values = date_range_inclusive(start, end)
        .into_iter()
        .map(|date| (date, 0))
        .collect::<BTreeMap<_, _>>();

    for activity in activities {
        if let Some(tss) = activity.metrics.training_stress_score {
            let date = activity_date(activity);
            if let Some(current) = values.get_mut(&date) {
                *current += tss;
            }
        }
    }

    values
}

pub(super) fn build_load_trend(
    values: &BTreeMap<NaiveDate, i32>,
    trend_days: usize,
    ctl_window_days: usize,
) -> Vec<HistoricalLoadTrendPoint> {
    let entries = values
        .iter()
        .map(|(date, tss)| (*date, *tss))
        .collect::<Vec<_>>();
    let start_index = entries.len().saturating_sub(trend_days);
    entries
        .iter()
        .enumerate()
        .skip(start_index)
        .map(|(index, (_, daily_tss))| {
            build_load_trend_point(&entries, index, ctl_window_days, 1, *daily_tss)
        })
        .collect()
}

pub(super) fn build_recent_interval_blocks_by_activity_id(
    detailed_activities: &[Activity],
    matched: &EventActivityMatches,
) -> HashMap<String, Vec<PlannedWorkoutBlockContext>> {
    detailed_activities
        .iter()
        .filter_map(|activity| {
            matched.activity_to_event.get(&activity.id).map(|event| {
                let parsed =
                    parse_workout_doc(event.workout_doc.as_deref(), activity.metrics.ftp_watts);
                (
                    activity.id.clone(),
                    event
                        .workout_doc
                        .as_ref()
                        .map(|_| {
                            build_recent_interval_blocks(
                                &parsed.segments,
                                activity.metrics.ftp_watts,
                            )
                        })
                        .unwrap_or_default(),
                )
            })
        })
        .collect()
}

fn build_recent_interval_blocks(
    segments: &[crate::domain::intervals::WorkoutSegment],
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

fn build_load_trend_point(
    entries: &[(NaiveDate, i32)],
    end_index: usize,
    ctl_window_days: usize,
    sample_days: u8,
    period_tss: i32,
) -> HistoricalLoadTrendPoint {
    let ctl = ewma_at_index(entries, end_index, ctl_window_days);
    let atl = ewma_at_index(entries, end_index, 7);

    HistoricalLoadTrendPoint {
        date: entries[end_index].0.format("%Y-%m-%d").to_string(),
        sample_days,
        period_tss,
        rolling_tss_7d: rolling_average_at_index(entries, end_index, 7),
        rolling_tss_28d: rolling_average_at_index(entries, end_index, 28),
        ctl,
        atl,
        tsb: match (ctl, atl) {
            (Some(ctl), Some(atl)) => Some(round_to_2(ctl - atl)),
            _ => None,
        },
    }
}

pub(super) fn ewma_latest(
    values: &BTreeMap<NaiveDate, i32>,
    time_constant_days: usize,
) -> Option<f64> {
    let entries = values
        .iter()
        .map(|(date, tss)| (*date, *tss))
        .collect::<Vec<_>>();
    entries
        .len()
        .checked_sub(1)
        .and_then(|end_index| ewma_at_index(&entries, end_index, time_constant_days))
}

fn ewma_at_index(
    values: &[(NaiveDate, i32)],
    end_index: usize,
    time_constant_days: usize,
) -> Option<f64> {
    if values.is_empty() || time_constant_days == 0 || end_index >= values.len() {
        return None;
    }

    let alpha = 1.0 / time_constant_days as f64;
    let mut current = 0.0;
    for (_, tss) in values.iter().take(end_index + 1) {
        current += (*tss as f64 - current) * alpha;
    }

    Some(round_to_2(current))
}

fn rolling_average_at_index(
    values: &[(NaiveDate, i32)],
    end_index: usize,
    window_days: usize,
) -> Option<f64> {
    if values.is_empty() || end_index + 1 < window_days {
        return None;
    }

    let start_index = end_index + 1 - window_days;
    let total = values[start_index..=end_index]
        .iter()
        .map(|(_, value)| *value)
        .sum::<i32>();

    Some(round_to_2(total as f64 / window_days as f64))
}

pub(super) fn average_recent_tss(
    values: &BTreeMap<NaiveDate, i32>,
    window_days: usize,
) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let total = values.values().rev().take(window_days).sum::<i32>();
    let count = values.values().rev().take(window_days).count();
    if count == 0 {
        None
    } else {
        Some(round_to_2(total as f64 / count as f64))
    }
}

pub(super) fn average_metric(values: impl Iterator<Item = f64>) -> Option<f64> {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        None
    } else {
        Some(round_to_2(
            collected.iter().sum::<f64>() / collected.len() as f64,
        ))
    }
}

pub(super) fn recent_slice(activities: &[Activity], start: NaiveDate) -> Vec<&Activity> {
    activities
        .iter()
        .filter(|activity| activity_date(activity) >= start)
        .collect()
}
