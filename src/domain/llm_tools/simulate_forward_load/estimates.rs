use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::{
    intervals::{parse_workout_doc, serialize_planned_workout},
    training_context::{
        FuturePlannedEventContext, ProjectedDayContext, RaceContext, TrainingContext,
        UpcomingDayContext,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TssSource {
    Planned,
    Projected,
    Default,
    Estimated,
    None,
}

#[derive(Clone, Debug)]
pub(super) struct PlannedLoadEstimate {
    pub(super) tss: f64,
    pub(super) duration_seconds: Option<i32>,
    pub(super) source: String,
    pub(super) tss_source: TssSource,
    pub(super) emit_race_tss_unknown_note: bool,
    pub(super) is_rest: bool,
    pub(super) rest_reason: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Baseline {
    pub(super) ctl: f64,
    pub(super) atl: f64,
    pub(super) tsb: f64,
}

pub(super) fn snapshot_baseline(training_context: &TrainingContext) -> Baseline {
    let ctl = ctl_from_context(training_context);
    let atl = atl_from_context(training_context);
    let tsb = tsb_from_context(training_context);
    Baseline { ctl, atl, tsb }
}

pub(super) fn select_estimates_for_day(
    input_days: &[crate::domain::intervals::PlannedWorkoutDay],
    upcoming_days: &[UpcomingDayContext],
    projected_days: &[ProjectedDayContext],
    future_events: &[FuturePlannedEventContext],
    races: &[RaceContext],
    date: &str,
    ftp_watts: Option<i32>,
) -> Vec<PlannedLoadEstimate> {
    let input = input_day_estimate(input_days, date, ftp_watts);
    let upcoming = input
        .is_none()
        .then(|| upcoming_day_estimate(upcoming_days, date, ftp_watts))
        .flatten();
    let race_calendar = race_estimate(races, date);
    let skip_projected = race_calendar.is_some();
    let (future, race) =
        resolve_race_day_sources(future_event_estimate(future_events, date), race_calendar);
    let projected = (input.is_none() && !skip_projected)
        .then(|| projected_day_estimate(projected_days, date, ftp_watts))
        .flatten();

    [input, upcoming, projected, future, race]
        .into_iter()
        .flatten()
        .collect()
}

/// Prefer Intervals event load when it is planned/estimated; fall back to races calendar
/// when the event is unestimable. Never sum both for the same day (double-count).
fn resolve_race_day_sources(
    future: Option<PlannedLoadEstimate>,
    race: Option<PlannedLoadEstimate>,
) -> (Option<PlannedLoadEstimate>, Option<PlannedLoadEstimate>) {
    match (future, race) {
        (Some(future), Some(race)) => match future.tss_source {
            TssSource::None => (None, Some(race)),
            _ => (Some(future), None),
        },
        pair => pair,
    }
}

pub(super) fn combine_estimates(estimates: Vec<PlannedLoadEstimate>) -> PlannedLoadEstimate {
    if estimates.is_empty() {
        return PlannedLoadEstimate {
            tss: 0.0,
            duration_seconds: None,
            source: "empty".to_string(),
            tss_source: TssSource::Default,
            emit_race_tss_unknown_note: false,
            is_rest: true,
            rest_reason: Some("no planned load".to_string()),
        };
    }

    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut sources = Vec::new();
    let mut is_rest_day = true;
    let mut rest_reason: Option<String> = None;
    let mut emit_race_tss_unknown_note = false;
    let mut non_event_tss_source: Option<TssSource> = None;
    let mut event_tss_source: Option<TssSource> = None;

    for estimate in estimates {
        total_tss += estimate.tss;
        if let Some(d) = estimate.duration_seconds {
            total_duration += d;
        }
        if is_race_day_source(&estimate.source) {
            event_tss_source = Some(estimate.tss_source);
        } else {
            non_event_tss_source.get_or_insert(estimate.tss_source);
        }
        emit_race_tss_unknown_note |= estimate.emit_race_tss_unknown_note;
        sources.push(estimate.source);
        if !estimate.is_rest {
            is_rest_day = false;
        }
        if estimate.is_rest && rest_reason.is_none() {
            rest_reason = estimate.rest_reason;
        }
    }

    let source_label = if sources.len() == 1 {
        sources.into_iter().next().unwrap()
    } else {
        sources.join("+")
    };

    let tss_source = if emit_race_tss_unknown_note || event_tss_source == Some(TssSource::None) {
        TssSource::None
    } else {
        non_event_tss_source
            .or(event_tss_source)
            .unwrap_or(TssSource::Default)
    };

    PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: source_label,
        tss_source,
        emit_race_tss_unknown_note,
        is_rest: is_rest_day,
        rest_reason: if is_rest_day { rest_reason } else { None },
    }
}

fn input_day_estimate(
    days: &[crate::domain::intervals::PlannedWorkoutDay],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let day = days.iter().find(|day| day.date == date)?;
    if day.is_rest_day() {
        return Some(PlannedLoadEstimate {
            tss: 0.0,
            duration_seconds: None,
            source: "input".to_string(),
            tss_source: TssSource::Planned,
            emit_race_tss_unknown_note: false,
            is_rest: true,
            rest_reason: day.rest_day_reason().map(str::to_string),
        });
    }

    let workout = day.planned_workout()?;
    let raw = serialize_planned_workout(workout);
    let parsed = parse_workout_doc(Some(raw.as_str()), ftp_watts);

    Some(PlannedLoadEstimate {
        tss: parsed
            .summary
            .estimated_training_stress_score
            .unwrap_or(0.0),
        duration_seconds: Some(parsed.summary.total_duration_seconds),
        source: "input".to_string(),
        tss_source: TssSource::Planned,
        emit_race_tss_unknown_note: false,
        is_rest: false,
        rest_reason: None,
    })
}

fn upcoming_day_estimate(
    upcoming_days: &[UpcomingDayContext],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let upcoming = upcoming_days.iter().find(|day| day.date == date)?;

    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut has_any = false;

    for workout in &upcoming.planned_workouts {
        has_any = true;
        let parsed = workout
            .raw_workout_doc
            .as_deref()
            .map(|raw| parse_workout_doc(Some(raw), ftp_watts));

        total_tss += workout
            .estimated_training_stress_score
            .or_else(|| {
                parsed
                    .as_ref()
                    .and_then(|p| p.summary.estimated_training_stress_score)
            })
            .unwrap_or(0.0);

        if let Some(parsed) = parsed.as_ref() {
            total_duration += parsed.summary.total_duration_seconds;
        }
    }

    if !has_any {
        return None;
    }

    Some(PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: "upcoming".to_string(),
        tss_source: TssSource::Planned,
        emit_race_tss_unknown_note: false,
        is_rest: false,
        rest_reason: None,
    })
}

fn projected_day_estimate(
    projected_days: &[ProjectedDayContext],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let projected = projected_days
        .iter()
        .find(|projected| projected.date == date)?;

    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut rest_reason: Option<String> = None;
    let mut has_any = false;

    for workout in &projected.workouts {
        if workout.rest_day {
            if rest_reason.is_none() {
                rest_reason = workout.rest_day_reason.clone();
            }
            continue;
        }

        has_any = true;

        if let Some(raw) = workout.raw_workout_doc.as_deref() {
            let parsed = parse_workout_doc(Some(raw), ftp_watts);
            total_tss += parsed
                .summary
                .estimated_training_stress_score
                .unwrap_or(0.0);
            total_duration += parsed.summary.total_duration_seconds;
        }
    }

    if !has_any {
        return Some(PlannedLoadEstimate {
            tss: 0.0,
            duration_seconds: None,
            source: "projected".to_string(),
            tss_source: TssSource::Projected,
            emit_race_tss_unknown_note: false,
            is_rest: true,
            rest_reason,
        });
    }

    Some(PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: "projected".to_string(),
        tss_source: TssSource::Projected,
        emit_race_tss_unknown_note: false,
        is_rest: false,
        rest_reason: None,
    })
}

fn race_estimate(races: &[RaceContext], date: &str) -> Option<PlannedLoadEstimate> {
    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut has_any = false;
    let mut any_unknown_tss = false;
    let mut any_estimated_tss = false;

    for race in races {
        if !race_date_matches(&race.date, date) {
            continue;
        }
        has_any = true;
        match duration_from_distance_meters(race.distance_meters) {
            Some(dur) => {
                total_duration += dur;
                total_tss += estimate_tss(dur, default_race_intensity_factor(&race.priority));
                any_estimated_tss = true;
            }
            None => any_unknown_tss = true,
        }
    }

    if !has_any {
        return None;
    }

    Some(event_load_estimate(
        "race",
        total_tss,
        total_duration,
        if any_unknown_tss {
            TssSource::None
        } else if any_estimated_tss {
            TssSource::Estimated
        } else {
            TssSource::None
        },
        any_unknown_tss,
    ))
}

fn future_event_estimate(
    future_events: &[FuturePlannedEventContext],
    date: &str,
) -> Option<PlannedLoadEstimate> {
    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut has_any = false;
    let mut any_unknown_tss = false;
    let mut any_estimated_tss = false;
    let mut any_planned_tss = false;

    for event in future_events {
        if event.start_date_local.get(..10) != Some(date) {
            continue;
        }
        has_any = true;
        let duration = event
            .estimated_duration_seconds
            .filter(|seconds| *seconds > 0)
            .or_else(|| duration_from_distance_description(event.description.as_deref()));
        if let Some(d) = duration {
            total_duration += d;
        }

        match event.estimated_training_stress_score {
            Some(tss) => {
                total_tss += tss;
                any_planned_tss = true;
            }
            None => match duration {
                Some(dur) => {
                    let intensity = event
                        .estimated_intensity_factor
                        .filter(|value| *value > 0.0)
                        .unwrap_or_else(|| default_race_intensity_factor(&event.category));
                    total_tss += estimate_tss(dur, intensity);
                    any_estimated_tss = true;
                }
                None => any_unknown_tss = true,
            },
        }
    }

    if !has_any {
        return None;
    }

    Some(event_load_estimate(
        "future_event",
        total_tss,
        total_duration,
        if any_unknown_tss {
            TssSource::None
        } else if any_estimated_tss {
            TssSource::Estimated
        } else if any_planned_tss {
            TssSource::Planned
        } else {
            TssSource::None
        },
        any_unknown_tss,
    ))
}

fn event_load_estimate(
    source: &str,
    tss: f64,
    total_duration: i32,
    tss_source: TssSource,
    emit_race_tss_unknown_note: bool,
) -> PlannedLoadEstimate {
    PlannedLoadEstimate {
        tss,
        duration_seconds: (total_duration > 0).then_some(total_duration),
        source: source.to_string(),
        tss_source,
        emit_race_tss_unknown_note,
        is_rest: false,
        rest_reason: None,
    }
}

fn is_race_day_source(source: &str) -> bool {
    source == "future_event" || source == "race"
}

fn race_date_matches(race_date: &str, date: &str) -> bool {
    race_date.get(..10).unwrap_or(race_date) == date
}

/// Accepts Intervals categories (`RACE_A`) and race-calendar priorities (`A` / `a`).
fn default_race_intensity_factor(label: &str) -> f64 {
    let trimmed = label.trim();
    let normalized = trimmed
        .strip_prefix("RACE_")
        .or_else(|| trimmed.strip_prefix("race_"))
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    match normalized.as_str() {
        "A" => 0.90,
        "B" => 0.85,
        _ => 0.80,
    }
}

fn estimate_tss(duration_seconds: i32, intensity_factor: f64) -> f64 {
    (duration_seconds as f64 / 3600.0) * intensity_factor * intensity_factor * 100.0
}

fn duration_from_distance_meters(meters: i32) -> Option<i32> {
    (meters > 0).then(|| (f64::from(meters) * 0.12).round() as i32)
}

/// Race sync writes `distance_meters=N` into the Intervals event description.
fn duration_from_distance_description(description: Option<&str>) -> Option<i32> {
    let description = description?;
    let key = "distance_meters=";
    let start = description.find(key)? + key.len();
    let digits = description[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    let meters: i32 = digits.parse().ok()?;
    duration_from_distance_meters(meters)
}

fn ctl_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .ctl
        .or_else(|| {
            training_context
                .history
                .load_trend
                .last()
                .and_then(|point| point.ctl)
        })
        .unwrap_or_default()
}

fn atl_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .atl
        .or_else(|| {
            training_context
                .history
                .load_trend
                .last()
                .and_then(|point| point.atl)
        })
        .or_else(|| {
            training_context
                .history
                .tsb
                .map(|tsb| ctl_from_context(training_context) - tsb)
        })
        .unwrap_or_default()
}

fn tsb_from_context(training_context: &TrainingContext) -> f64 {
    training_context
        .history
        .tsb
        .unwrap_or_else(|| ctl_from_context(training_context) - atl_from_context(training_context))
}

pub(super) fn update_load(current: f64, planned_tss: f64, time_constant_days: f64) -> f64 {
    current + (planned_tss - current) * (1.0 / time_constant_days)
}

pub(super) fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub(super) fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

pub(super) fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
