use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::{
    intervals::{parse_workout_doc, serialize_planned_workout},
    training_context::{
        FuturePlannedEventContext, ProjectedDayContext, TrainingContext, UpcomingDayContext,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TssSource {
    Planned,
    Projected,
    Default,
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
    date: &str,
    ftp_watts: Option<i32>,
) -> Vec<PlannedLoadEstimate> {
    let input = input_day_estimate(input_days, date, ftp_watts);

    if input.is_some() {
        let future = future_event_estimate(future_events, date);
        [input, future].into_iter().flatten().collect()
    } else {
        let upcoming = upcoming_day_estimate(upcoming_days, date, ftp_watts);
        let projected = projected_day_estimate(projected_days, date, ftp_watts);
        let future = future_event_estimate(future_events, date);
        [upcoming, projected, future]
            .into_iter()
            .flatten()
            .collect()
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
        if estimate.source == "future_event" {
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

fn future_event_estimate(
    future_events: &[FuturePlannedEventContext],
    date: &str,
) -> Option<PlannedLoadEstimate> {
    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut has_any = false;
    let mut any_unknown_tss = false;

    for event in future_events {
        if event.start_date_local.get(..10) == Some(date) {
            has_any = true;
            match event.estimated_training_stress_score {
                Some(tss) => {
                    total_tss += tss;
                }
                None => {
                    any_unknown_tss = true;
                }
            }
            if let Some(d) = event.estimated_duration_seconds {
                total_duration += d;
            }
        }
    }

    if !has_any {
        return None;
    }

    // Any unknown same-day event TSS keeps provenance none even when other
    // events on that date have quantified load (unknown portion modelled as 0).
    let tss_source = if any_unknown_tss {
        TssSource::None
    } else {
        TssSource::Planned
    };

    Some(PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: "future_event".to_string(),
        tss_source,
        emit_race_tss_unknown_note: any_unknown_tss,
        is_rest: false,
        rest_reason: None,
    })
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
