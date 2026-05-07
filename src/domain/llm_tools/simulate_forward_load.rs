use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{
    intervals::{parse_planned_workout_days, parse_workout_doc, serialize_planned_workout},
    llm::LlmToolDefinition,
    training_context::{
        FuturePlannedEventContext, ProjectedDayContext, TrainingContext, UpcomingDayContext,
    },
};

use super::{LlmTool, ToolExecutionContext};

const SIMULATE_FORWARD_LOAD_TOOL_NAME: &str = "simulate_forward_load";

/// Estimate of planned load for a single day from one source.
#[derive(Clone, Debug)]
struct PlannedLoadEstimate {
    tss: f64,
    duration_seconds: Option<i32>,
    source: String,
    is_rest: bool,
    rest_reason: Option<String>,
}

#[derive(Deserialize)]
struct SimulateForwardLoadArgs {
    /// Dated workout text for days the LLM wants to override or add.
    /// If omitted, the simulation uses only context sources
    /// (upcoming days, projected days, future events).
    dated_workout_text: Option<String>,
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

/// Simulates forward training load for 14 days from today.
/// Automatically includes context sources (upcoming workouts, projected
/// workouts, future events). LLM only needs to provide `dated_workout_text`
/// for days it wants to override or add.
pub struct SimulateForwardLoad;

impl LlmTool for SimulateForwardLoad {
    fn name(&self) -> &'static str {
        SIMULATE_FORWARD_LOAD_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Simulate 14 days of forward training load from today. The tool automatically includes already-scheduled workouts (upcoming days), projected workouts, and future events (races). Only provide dated_workout_text for days you want to override or add new workouts.\n\nFormat: Each day starts with a YYYY-MM-DD header on its own line, followed by workout steps or 'Rest Day'. You can use section titles, ramps, repeat headers (Nx), and power targets in %FTP or watts.\n\nExample 1 - Simple:\n2026-05-05\n- 90m 65%\n2026-05-06\nRest Day: recovery\n\nExample 2 - Complex interval session:\n2026-05-07\nWarmup\n- 15m ramp 55-75%\n\nMain Set\n4x\n- 2m 105%\n- 1m 65%\n\n3x\n- 3m 95%\n- 2m 65%\n\nCooldown\n- 10m 55%".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dated_workout_text": {
                        "type": "string",
                        "description": "Optional. Dated workout text for days you want to override or add, in YYYY-MM-DD plus workout-builder format. If omitted, the simulation uses only existing scheduled workouts, projections, and events from context."
                    }
                }
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "use when reasoning about future fatigue, load progression, or the impact of planned workouts; prefer this over mental arithmetic from CTL/ATL/TSB alone",
        )
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let ctx = context.clone();
        Box::pin(async move { simulate_forward_load(&args, &ctx) })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: SimulateForwardLoadArgs = serde_json::from_str(arguments_json).ok()?;
        let text = args.dated_workout_text.as_deref()?;
        let parsed = parse_planned_workout_days(text).ok()?;
        let first = parsed.days.first()?.date.clone();
        let last = parsed.days.last()?.date.clone();
        let count = parsed.days.len();
        let day_word = if count == 1 { "day" } else { "days" };
        Some(format!("{count} dated {day_word} from {first} to {last}"))
    }
}

fn simulate_forward_load(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let args = match serde_json::from_str::<SimulateForwardLoadArgs>(arguments_json) {
        Ok(args) => args,
        Err(error) => {
            return json!({
                "error": format!("invalid simulate_forward_load arguments: {error}")
            })
            .to_string();
        }
    };

    let input_days = args
        .dated_workout_text
        .as_deref()
        .and_then(|text| parse_planned_workout_days(text).ok())
        .map(|parsed| parsed.days)
        .unwrap_or_default();

    let Some(today) = parse_date(&context.today) else {
        return json!({
            "error": format!("invalid today date in tool context: {}", context.today)
        })
        .to_string();
    };

    let ftp_watts = context.training_context.history.ftp_current;
    let baseline = snapshot_baseline(&context.training_context);
    let (mut ctl, mut atl) = (baseline.ctl, baseline.atl);
    let mut days = Vec::with_capacity(14);

    for offset in 1..=14 {
        let date = today + Duration::days(offset);
        let date_key = format_date(date);

        let estimates = select_estimates_for_day(
            &input_days,
            &context.training_context.upcoming_days,
            &context.training_context.projected_days,
            &context.training_context.future_events,
            &date_key,
            ftp_watts,
        );

        let combined = combine_estimates(estimates);

        ctl = update_load(ctl, combined.tss, 42.0);
        atl = update_load(atl, combined.tss, 7.0);
        let tsb = round_to_2(ctl - atl);

        days.push(ForwardLoadDay {
            date: date_key,
            planned_tss: round_to_2(combined.tss),
            planned_duration_seconds: combined.duration_seconds,
            source: combined.source,
            rest_day: combined.is_rest,
            rest_day_reason: combined.rest_reason,
            ctl: round_to_2(ctl),
            atl: round_to_2(atl),
            tsb,
        });
    }

    json!(SimulateForwardLoadResponse {
        baseline: ForwardLoadBaseline {
            today: context.today.clone(),
            ctl: round_to_2(baseline.ctl),
            atl: round_to_2(baseline.atl),
            tsb: round_to_2(baseline.tsb),
            ftp_watts,
        },
        days,
    })
    .to_string()
}

#[derive(Clone, Copy, Debug)]
struct Baseline {
    ctl: f64,
    atl: f64,
    tsb: f64,
}

fn snapshot_baseline(training_context: &TrainingContext) -> Baseline {
    let ctl = ctl_from_context(training_context);
    let atl = atl_from_context(training_context);
    let tsb = tsb_from_context(training_context);
    Baseline { ctl, atl, tsb }
}

/// Gather load estimates for a single day from all available sources.
/// Input from LLM overrides upcoming/projected; future events are always additive.
fn select_estimates_for_day(
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

/// Combine multiple source estimates into a single day summary.
fn combine_estimates(estimates: Vec<PlannedLoadEstimate>) -> PlannedLoadEstimate {
    if estimates.is_empty() {
        return PlannedLoadEstimate {
            tss: 0.0,
            duration_seconds: None,
            source: "empty".to_string(),
            is_rest: true,
            rest_reason: Some("no planned load".to_string()),
        };
    }

    let mut total_tss = 0.0;
    let mut total_duration: i32 = 0;
    let mut sources = Vec::new();
    let mut is_rest_day = true;
    let mut rest_reason: Option<String> = None;

    for estimate in estimates {
        total_tss += estimate.tss;
        if let Some(d) = estimate.duration_seconds {
            total_duration += d;
        }
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

    PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: source_label,
        is_rest: is_rest_day,
        rest_reason,
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
        is_rest: false,
        rest_reason: None,
    })
}

/// Aggregate all already-scheduled workouts for an upcoming day.
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
        is_rest: false,
        rest_reason: None,
    })
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
        return Some(PlannedLoadEstimate {
            tss: 0.0,
            duration_seconds: None,
            source: "projected".to_string(),
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
        is_rest: false,
        rest_reason: None,
    })
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

    Some(PlannedLoadEstimate {
        tss: total_tss,
        duration_seconds: if total_duration > 0 {
            Some(total_duration)
        } else {
            None
        },
        source: "future_event".to_string(),
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
    use super::SimulateForwardLoad;
    use crate::domain::llm_tools::{LlmTool, ToolExecutionContext};
    use crate::domain::training_context::PlannedWorkoutContext;
    use crate::domain::training_context::{
        AthleteProfileContext, ProjectedDayContext, ProjectedWorkoutContext, TrainingContext,
        UpcomingDayContext,
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
            upcoming_days: vec![UpcomingDayContext {
                date: "2026-05-05".to_string(),
                free_day: false,
                planned_workouts: vec![PlannedWorkoutContext {
                    event_id: 1,
                    start_date_local: "2026-05-05T06:00:00".to_string(),
                    name: Some("Morning Endurance".to_string()),
                    category: "workout".to_string(),
                    interval_blocks: Vec::new(),
                    raw_workout_doc: Some("- 60m 65%".to_string()),
                    estimated_training_stress_score: Some(65.0),
                    estimated_intensity_factor: None,
                    estimated_normalized_power_watts: None,
                    completed: false,
                }],
                special_days: Vec::new(),
            }],
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
            user_id: "user-1".to_string(),
            training_context,
            today: "2026-05-04".to_string(),
            data_port: None,
        }
    }

    #[test]
    fn preview_tool_arguments_summarizes_dated_text_range() {
        let tool = SimulateForwardLoad;
        let preview = tool.preview_arguments(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day"}"#,
        );

        assert_eq!(
            preview.as_deref(),
            Some("2 dated days from 2026-05-05 to 2026-05-06")
        );
    }

    #[test]
    fn preview_tool_arguments_uses_singular_day() {
        let tool = SimulateForwardLoad;
        let preview = tool.preview_arguments(r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#);

        assert_eq!(
            preview.as_deref(),
            Some("1 dated day from 2026-05-05 to 2026-05-05")
        );
    }

    #[test]
    fn simulate_forward_load_returns_14_day_sequence() {
        let tool = SimulateForwardLoad;
        let response = futures::executor::block_on(tool.execute(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day: absorb load"}"#,
            &sample_context(),
        ));

        assert!(response.contains("\"baseline\""));
        assert!(response.contains("\"2026-05-05\""));
        assert!(response.contains("\"source\":\"input\""));
        assert!(response.contains("\"source\":\"projected\""));
        assert!(response.contains("\"2026-05-18\""));
    }

    #[test]
    fn simulate_forward_load_aggregates_multiple_projected_workouts() {
        let tool = SimulateForwardLoad;
        let response = futures::executor::block_on(tool.execute(
            r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#,
            &sample_context(),
        ));

        // 2026-05-06 has two projected workouts (90m 65% + 30m easy spin).
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

        let tool = SimulateForwardLoad;
        let response = futures::executor::block_on(
            tool.execute(r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#, &ctx),
        );

        assert!(response.contains("\"2026-05-08\""));
        assert!(response.contains("\"source\":\"future_event\""));
    }

    #[test]
    fn simulate_forward_load_without_input_uses_context_sources() {
        let tool = SimulateForwardLoad;
        let response = futures::executor::block_on(tool.execute(r#"{}"#, &sample_context()));

        // 2026-05-05 should have "upcoming" source (from calendar)
        assert!(response.contains("\"2026-05-05\""));
        assert!(response.contains("\"source\":\"upcoming\""));

        // 2026-05-06 should have "projected" source
        assert!(response.contains("\"2026-05-06\""));
        assert!(response.contains("\"source\":\"projected\""));
    }

    #[test]
    fn simulate_forward_load_input_overrides_upcoming() {
        let ctx = sample_context();
        let tool = SimulateForwardLoad;
        let response = futures::executor::block_on(
            tool.execute(r#"{"dated_workout_text":"2026-05-05\n- 120m 60%"}"#, &ctx),
        );

        // 2026-05-05 should use input, not upcoming
        assert!(response.contains("\"2026-05-05\""));
        assert!(response.contains("\"source\":\"input\""));
    }
}
