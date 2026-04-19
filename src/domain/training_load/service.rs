use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};

use crate::domain::completed_workouts::CompletedWorkout;

use super::{FtpHistoryEntry, FtpSource, TrainingLoadDailySnapshot, TrainingLoadSnapshotRange};

const CTL_TIME_CONSTANT_DAYS: usize = 42;
const ATL_TIME_CONSTANT_DAYS: usize = 7;

pub fn build_daily_training_load_snapshots(
    user_id: &str,
    range: &TrainingLoadSnapshotRange,
    workouts: &[CompletedWorkout],
    ftp_history: &[FtpHistoryEntry],
    app_entry_date: &str,
    now_epoch_seconds: i64,
) -> Vec<TrainingLoadDailySnapshot> {
    let Some(start) = parse_date(&range.oldest) else {
        return Vec::new();
    };
    let Some(end) = parse_date(&range.newest) else {
        return Vec::new();
    };
    if start > end {
        return Vec::new();
    }

    let app_entry_date = parse_date(app_entry_date).unwrap_or(start);
    let ordered_history = sort_history(ftp_history);
    let mut daily_tss = date_range_inclusive(start, end)
        .into_iter()
        .map(|date| (date, 0))
        .collect::<BTreeMap<_, _>>();

    for workout in workouts {
        let Some(date) = workout_date(workout) else {
            continue;
        };
        if date < start || date > end {
            continue;
        }

        let effective_ftp = effective_history_entry_for_date(&ordered_history, date);
        if let Some(tss) =
            resolve_training_stress_score(workout, effective_ftp, app_entry_date, date)
        {
            if let Some(current) = daily_tss.get_mut(&date) {
                *current += tss;
            }
        }
    }

    let ordered_days = daily_tss
        .iter()
        .map(|(date, tss)| (*date, *tss))
        .collect::<Vec<_>>();

    ordered_days
        .iter()
        .enumerate()
        .map(|(index, (date, day_tss))| {
            let date_key = format_date(*date);
            let day_workouts = workouts_for_date(workouts, *date);
            let effective_ftp = effective_history_entry_for_date(&ordered_history, *date);
            let (ftp_effective_watts, ftp_source) = match effective_ftp {
                Some(entry) if entry.ftp_watts > 0 => {
                    (Some(entry.ftp_watts), Some(entry.source.clone()))
                }
                None => {
                    let provider_ftp = day_workouts
                        .iter()
                        .filter_map(|workout| workout.metrics.ftp_watts)
                        .next_back();
                    (provider_ftp, provider_ftp.map(|_| FtpSource::Provider))
                }
                Some(_) => {
                    let provider_ftp = day_workouts
                        .iter()
                        .filter_map(|workout| workout.metrics.ftp_watts)
                        .next_back();
                    (provider_ftp, provider_ftp.map(|_| FtpSource::Provider))
                }
            };

            TrainingLoadDailySnapshot {
                user_id: user_id.to_string(),
                date: date_key,
                daily_tss: Some(*day_tss),
                rolling_tss_7d: rolling_average_at_index(&ordered_days, index, 7),
                rolling_tss_28d: rolling_average_at_index(&ordered_days, index, 28),
                ctl: ewma_at_index(&ordered_days, index, CTL_TIME_CONSTANT_DAYS),
                atl: ewma_at_index(&ordered_days, index, ATL_TIME_CONSTANT_DAYS),
                tsb: match (
                    ewma_at_index(&ordered_days, index, CTL_TIME_CONSTANT_DAYS),
                    ewma_at_index(&ordered_days, index, ATL_TIME_CONSTANT_DAYS),
                ) {
                    (Some(ctl), Some(atl)) => Some(round_to_2(ctl - atl)),
                    _ => None,
                },
                average_if_28d: average_metric(
                    recent_workouts_until(workouts, *date, 28)
                        .into_iter()
                        .filter_map(|workout| workout.metrics.intensity_factor),
                ),
                average_ef_28d: average_metric(
                    recent_workouts_until(workouts, *date, 28)
                        .into_iter()
                        .filter_map(|workout| workout.metrics.efficiency_factor),
                ),
                ftp_effective_watts,
                ftp_source,
                recomputed_at_epoch_seconds: now_epoch_seconds,
                created_at_epoch_seconds: now_epoch_seconds,
                updated_at_epoch_seconds: now_epoch_seconds,
            }
        })
        .collect()
}

fn sort_history(history: &[FtpHistoryEntry]) -> Vec<FtpHistoryEntry> {
    let mut values = history.to_vec();
    values.sort_by(|left, right| left.effective_from_date.cmp(&right.effective_from_date));
    values
}

fn effective_history_entry_for_date(
    history: &[FtpHistoryEntry],
    date: NaiveDate,
) -> Option<&FtpHistoryEntry> {
    let date_key = format_date(date);
    history
        .iter()
        .filter(|entry| entry.effective_from_date <= date_key)
        .max_by_key(|entry| entry.effective_from_date.as_str())
}

fn resolve_training_stress_score(
    workout: &CompletedWorkout,
    effective_ftp: Option<&FtpHistoryEntry>,
    app_entry_date: NaiveDate,
    workout_date: NaiveDate,
) -> Option<i32> {
    if workout_date < app_entry_date {
        return compute_tss_from_ftp(workout, workout.metrics.ftp_watts)
            .or(workout.metrics.training_stress_score);
    }

    let effective_ftp = effective_ftp
        .map(|entry| entry.ftp_watts)
        .filter(|ftp| *ftp > 0);

    compute_tss_from_ftp(workout, effective_ftp).or(workout.metrics.training_stress_score)
}

fn compute_tss_from_ftp(workout: &CompletedWorkout, ftp_watts: Option<i32>) -> Option<i32> {
    let effective_ftp = ftp_watts.filter(|ftp| *ftp > 0)?;
    let duration_seconds = workout.duration_seconds.filter(|seconds| *seconds > 0)?;
    let normalized_power_watts = workout
        .metrics
        .normalized_power_watts
        .filter(|watts| *watts > 0)?;

    let intensity_factor = normalized_power_watts as f64 / effective_ftp as f64;
    Some(round_to_i32(
        duration_seconds as f64 / 3600.0 * intensity_factor * intensity_factor * 100.0,
    ))
}

fn workouts_for_date(workouts: &[CompletedWorkout], date: NaiveDate) -> Vec<&CompletedWorkout> {
    workouts
        .iter()
        .filter(|workout| workout_date(workout) == Some(date))
        .collect()
}

fn recent_workouts_until(
    workouts: &[CompletedWorkout],
    end: NaiveDate,
    window_days: i64,
) -> Vec<&CompletedWorkout> {
    let start = end - Duration::days(window_days - 1);
    workouts
        .iter()
        .filter(|workout| {
            let Some(date) = workout_date(workout) else {
                return false;
            };
            date >= start && date <= end
        })
        .collect()
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

fn average_metric(values: impl Iterator<Item = f64>) -> Option<f64> {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        None
    } else {
        Some(round_to_2(
            collected.iter().sum::<f64>() / collected.len() as f64,
        ))
    }
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn workout_date(workout: &CompletedWorkout) -> Option<NaiveDate> {
    parse_date(workout.start_date_local.get(..10)?)
}

fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn date_range_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let days = (end - start).num_days().max(0);
    (0..=days)
        .filter_map(|offset| start.checked_add_signed(Duration::days(offset)))
        .collect()
}

fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round_to_i32(value: f64) -> i32 {
    value.round() as i32
}
