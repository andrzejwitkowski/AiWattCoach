use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{
    intervals::{parse_planned_workout_days, parse_workout_doc, serialize_planned_workout},
    training_context::{FuturePlannedEventContext, ProjectedDayContext, TrainingContext},
};

use super::SIMULATE_FORWARD_LOAD_TOOL_NAME;

type PlannedLoadEstimate = (f64, Option<i32>, String, bool, Option<String>);

#[derive(Deserialize)]
pub(super) struct SimulateForwardLoadArgs {
    dated_workout_text: String,
}

#[derive(Serialize)]
struct SimulateForwardLoadResponse {
    baseline: ForwardLoadBaseline,
    days: Vec<ForwardLoadDay>,
}

#[derive(Serialize)]
struct ForwardLoadBaseline {
    today: String,
    ctl: f64,
    atl: f64,
    tsb: f64,
    ftp_watts: Option<i32>,
}

#[derive(Serialize)]
struct ForwardLoadDay {
    date: String,
    planned_tss: f64,
    planned_duration_seconds: Option<i32>,
    source: String,
    rest_day: bool,
    rest_day_reason: Option<String>,
    ctl: f64,
    atl: f64,
    tsb: f64,
}

pub(super) fn simulate_forward_load(
    arguments_json: &str,
    context: &super::ToolExecutionContext,
) -> String {
    let args = match serde_json::from_str::<SimulateForwardLoadArgs>(arguments_json) {
        Ok(args) => args,
        Err(error) => {
            return json!({
                "error": format!("invalid simulate_forward_load arguments: {error}")
            })
            .to_string();
        }
    };

    let parsed_days = match parse_planned_workout_days(&args.dated_workout_text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return json!({
                "error": format!("failed to parse dated workout text: {error}")
            })
            .to_string();
        }
    };

    let Some(today) = parse_date(&context.today) else {
        return json!({
            "error": format!("invalid today date in tool context: {}", context.today)
        })
        .to_string();
    };

    let ftp_watts = context.training_context.history.ftp_current;
    let (mut ctl, mut atl) = resolve_load_baseline(&context.training_context);
    let mut days = Vec::with_capacity(14);

    for offset in 1..=14 {
        let date = today + Duration::days(offset);
        let date_key = format_date(date);

        // Collect contributions from all sources and aggregate them.
        let input = input_day_estimate(&parsed_days.days, &date_key, ftp_watts);
        let projected = projected_day_estimate(
            &context.training_context.projected_days,
            &date_key,
            ftp_watts,
        );
        let future = future_event_estimate(&context.training_context.future_events, &date_key);

        let mut total_tss = 0.0;
        let mut total_duration: i32 = 0;
        let mut sources = Vec::new();
        let mut is_rest_day = true;
        let mut rest_reason: Option<String> = None;

        for estimate in [input, projected, future].into_iter().flatten() {
            let (tss, duration, source, rest, reason) = estimate;
            total_tss += tss;
            if let Some(d) = duration {
                total_duration += d;
            }
            sources.push(source);
            if !rest {
                is_rest_day = false;
            }
            if rest && rest_reason.is_none() {
                rest_reason = reason;
            }
        }

        let (planned_tss, planned_duration_seconds, source, rest_day, rest_day_reason) =
            if sources.is_empty() {
                (
                    0.0,
                    None,
                    "empty".to_string(),
                    true,
                    Some("no planned load".to_string()),
                )
            } else {
                let source_label = if sources.len() == 1 {
                    sources.into_iter().next().unwrap()
                } else {
                    sources.join("+")
                };
                (
                    total_tss,
                    if total_duration > 0 {
                        Some(total_duration)
                    } else {
                        None
                    },
                    source_label,
                    is_rest_day,
                    rest_reason,
                )
            };

        ctl = update_load(ctl, planned_tss, 42.0);
        atl = update_load(atl, planned_tss, 7.0);
        let tsb = round_to_2(ctl - atl);

        days.push(ForwardLoadDay {
            date: date_key,
            planned_tss: round_to_2(planned_tss),
            planned_duration_seconds,
            source,
            rest_day,
            rest_day_reason,
            ctl: round_to_2(ctl),
            atl: round_to_2(atl),
            tsb,
        });
    }

    json!(SimulateForwardLoadResponse {
        baseline: ForwardLoadBaseline {
            today: context.today.clone(),
            ctl: round_to_2(ctl_from_context(&context.training_context)),
            atl: round_to_2(atl_from_context(&context.training_context)),
            tsb: round_to_2(tsb_from_context(&context.training_context)),
            ftp_watts,
        },
        days,
    })
    .to_string()
}

pub(super) fn preview_tool_arguments(tool_name: &str, arguments_json: &str) -> Option<String> {
    match tool_name {
        SIMULATE_FORWARD_LOAD_TOOL_NAME => {
            let args: SimulateForwardLoadArgs = serde_json::from_str(arguments_json).ok()?;
            let parsed = parse_planned_workout_days(&args.dated_workout_text).ok()?;
            let first = parsed.days.first()?.date.clone();
            let last = parsed.days.last()?.date.clone();
            let count = parsed.days.len();
            let day_word = if count == 1 { "day" } else { "days" };
            Some(format!("{count} dated {day_word} from {first} to {last}"))
        }
        _ => None,
    }
}

fn input_day_estimate(
    days: &[crate::domain::intervals::PlannedWorkoutDay],
    date: &str,
    ftp_watts: Option<i32>,
) -> Option<PlannedLoadEstimate> {
    let day = days.iter().find(|day| day.date == date)?;
    if day.is_rest_day() {
        return Some((
            0.0,
            None,
            "input".to_string(),
            true,
            day.rest_day_reason().map(str::to_string),
        ));
    }

    let workout = day.planned_workout()?;
    let raw = serialize_planned_workout(workout);
    let parsed = parse_workout_doc(Some(raw.as_str()), ftp_watts);

    Some((
        parsed
            .summary
            .estimated_training_stress_score
            .unwrap_or(0.0),
        Some(parsed.summary.total_duration_seconds),
        "input".to_string(),
        false,
        None,
    ))
}

/// Aggregate all workouts for a projected day.  Only returns a rest-day
/// estimate when every workout on that day is a rest placeholder.
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
        return Some((0.0, None, "projected".to_string(), true, rest_reason));
    }

    Some((
        total_tss,
        if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        "projected".to_string(),
        false,
        None,
    ))
}

/// Sum all future events that fall on the requested date.
fn future_event_estimate(
    future_events: &[FuturePlannedEventContext],
    date: &str,
) -> Option<PlannedLoadEstimate> {
    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut has_any = false;

    for event in future_events {
        if event.start_date_local.get(..10) == Some(date) {
            has_any = true;
            total_tss += event.estimated_training_stress_score.unwrap_or(0.0);
            if let Some(d) = event.estimated_duration_seconds {
                total_duration += d;
            }
        }
    }

    if !has_any {
        return None;
    }

    Some((
        total_tss,
        if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        "future_event".to_string(),
        false,
        None,
    ))
}

fn resolve_load_baseline(training_context: &TrainingContext) -> (f64, f64) {
    (
        ctl_from_context(training_context),
        atl_from_context(training_context),
    )
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

fn update_load(current: f64, planned_tss: f64, time_constant_days: f64) -> f64 {
    current + (planned_tss - current) * (1.0 / time_constant_days)
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn round_to_2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{preview_tool_arguments, simulate_forward_load};
    use crate::domain::llm_tools::ToolExecutionContext;
    use crate::domain::training_context::{
        AthleteProfileContext, ProjectedDayContext, ProjectedWorkoutContext, TrainingContext,
    };

    fn sample_context() -> ToolExecutionContext {
        let mut training_context = TrainingContext {
            generated_at_epoch_seconds: 1_700_000_000,
            focus_workout_id: None,
            focus_kind: "summary".to_string(),
            intervals_status: Default::default(),
            profile: AthleteProfileContext {
                availability_configured: true,
                ..Default::default()
            },
            races: Vec::new(),
            future_events: Vec::new(),
            history: Default::default(),
            recent_days: Vec::new(),
            upcoming_days: Vec::new(),
            projected_days: vec![
                ProjectedDayContext {
                    date: "2026-05-06".to_string(),
                    workouts: vec![
                        ProjectedWorkoutContext {
                            source_workout_id: "planned-1a".to_string(),
                            start_date_local: "2026-05-06T06:00:00".to_string(),
                            name: Some("Projected Endurance".to_string()),
                            interval_blocks: Vec::new(),
                            raw_workout_doc: Some("- 90m 65%".to_string()),
                            rest_day: false,
                            rest_day_reason: None,
                        },
                        ProjectedWorkoutContext {
                            source_workout_id: "planned-1b".to_string(),
                            start_date_local: "2026-05-06T18:00:00".to_string(),
                            name: Some("Projected Drills".to_string()),
                            interval_blocks: Vec::new(),
                            raw_workout_doc: Some("- 30m easy spin".to_string()),
                            rest_day: false,
                            rest_day_reason: None,
                        },
                    ],
                },
                ProjectedDayContext {
                    date: "2026-05-07".to_string(),
                    workouts: vec![ProjectedWorkoutContext {
                        source_workout_id: "planned-2".to_string(),
                        start_date_local: "2026-05-07T06:00:00".to_string(),
                        name: Some("Projected Tempo".to_string()),
                        interval_blocks: Vec::new(),
                        raw_workout_doc: Some("- 60m 75%".to_string()),
                        rest_day: false,
                        rest_day_reason: None,
                    }],
                },
            ],
        };
        training_context.history.ctl = Some(72.0);
        training_context.history.atl = Some(81.0);
        training_context.history.tsb = Some(-9.0);
        training_context.history.ftp_current = Some(300);

        ToolExecutionContext {
            training_context,
            today: "2026-05-04".to_string(),
        }
    }

    #[test]
    fn preview_tool_arguments_summarizes_dated_text_range() {
        let preview = preview_tool_arguments(
            "simulate_forward_load",
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day"}"#,
        );

        assert_eq!(
            preview.as_deref(),
            Some("2 dated days from 2026-05-05 to 2026-05-06")
        );
    }

    #[test]
    fn preview_tool_arguments_uses_singular_day() {
        let preview = preview_tool_arguments(
            "simulate_forward_load",
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#,
        );

        assert_eq!(
            preview.as_deref(),
            Some("1 dated day from 2026-05-05 to 2026-05-05")
        );
    }

    #[test]
    fn simulate_forward_load_returns_14_day_sequence() {
        let response = simulate_forward_load(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day: absorb load"}"#,
            &sample_context(),
        );

        assert!(response.contains("\"baseline\""));
        assert!(response.contains("\"2026-05-05\""));
        assert!(response.contains("\"source\":\"input\""));
        assert!(response.contains("\"source\":\"projected\""));
        assert!(response.contains("\"2026-05-18\""));
    }

    #[test]
    fn simulate_forward_load_aggregates_multiple_projected_workouts() {
        let response = simulate_forward_load(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#,
            &sample_context(),
        );

        // 2026-05-06 has two projected workouts (90m 65% + 30m easy spin).
        // The response should contain this date with "projected" source.
        assert!(response.contains("\"2026-05-06\""));
        assert!(response.contains("\"source\":\"projected\""));
    }

    #[test]
    fn simulate_forward_load_adds_future_event_tss() {
        let mut ctx = sample_context();
        ctx.training_context.future_events = vec![
            crate::domain::training_context::FuturePlannedEventContext {
                event_id: 1,
                start_date_local: "2026-05-08T08:00:00".to_string(),
                category: "race".to_string(),
                event_type: Some("road".to_string()),
                name: Some("Test Race".to_string()),
                description: None,
                estimated_duration_seconds: Some(7200),
                estimated_training_stress_score: Some(150.0),
                estimated_intensity_factor: None,
                estimated_normalized_power_watts: None,
            },
            crate::domain::training_context::FuturePlannedEventContext {
                event_id: 2,
                start_date_local: "2026-05-08T14:00:00".to_string(),
                category: "race".to_string(),
                event_type: Some("tt".to_string()),
                name: Some("Stage 2".to_string()),
                description: None,
                estimated_duration_seconds: Some(3600),
                estimated_training_stress_score: Some(90.0),
                estimated_intensity_factor: None,
                estimated_normalized_power_watts: None,
            },
        ];

        let response =
            simulate_forward_load(r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#, &ctx);

        // 2026-05-08 should have source including "future_event"
        assert!(response.contains("\"2026-05-08\""));
        assert!(response.contains("\"source\":\"future_event\""));
    }
}
