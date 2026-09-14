use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{intervals::parse_planned_workout_days, llm::LlmToolDefinition};

use super::{LlmTool, ToolExecutionContext};

mod estimates;
#[cfg(test)]
mod tests;

use estimates::{
    combine_estimates, format_date, parse_date, round_to_2, select_estimates_for_day,
    snapshot_baseline, update_load, TssSource,
};

const SIMULATE_FORWARD_LOAD_TOOL_NAME: &str = "simulate_forward_load";

#[derive(Deserialize)]
struct SimulateForwardLoadArgs {
    dated_workout_text: Option<String>,
}

#[derive(Serialize)]
struct SimulateForwardLoadResponse {
    baseline: ForwardLoadBaseline,
    days: Vec<ForwardLoadDay>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
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
    tss_source: TssSource,
    rest_day: bool,
    rest_day_reason: Option<String>,
    ctl: f64,
    atl: f64,
    tsb: f64,
}

pub struct SimulateForwardLoad;

impl LlmTool for SimulateForwardLoad {
    fn name(&self) -> &'static str {
        SIMULATE_FORWARD_LOAD_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Simulate 14 days of forward training load from today. The tool automatically includes already-scheduled workouts (upcoming days), projected workouts, and future events (races). Only provide dated_workout_text for days you want to override or add new workouts.\n\nEach day includes tss_source (planned|projected|default|none). When a race/event has no quantified TSS, tss_source is none, modelled TSB still appears, and notes explain that race load was assumed zero.\n\nFormat: Each day starts with a YYYY-MM-DD header on its own line, followed by workout steps or 'Rest Day'. You can use section titles, ramps, repeat headers (Nx), and power targets in %FTP or watts.\n\nExample 1 - Simple:\n2026-05-05\n- 90m 65%\n2026-05-06\nRest Day: recovery\n\nExample 2 - Complex interval session:\n2026-05-07\nWarmup\n- 15m ramp 55-75%\n\nMain Set\n4x\n- 2m 105%\n- 1m 65%\n\n3x\n- 3m 95%\n- 2m 65%\n\nCooldown\n- 10m 55%".to_string(),
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

    let input_days = match args.dated_workout_text.as_deref() {
        None => Vec::new(),
        Some(text) => match parse_planned_workout_days(text) {
            Ok(parsed) => parsed.days,
            Err(error) => {
                return json!({
                    "error": format!("invalid dated_workout_text: {error}")
                })
                .to_string();
            }
        },
    };

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
    let mut notes = Vec::new();

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
        if combined.emit_race_tss_unknown_note {
            notes.push(format!(
                "{date_key}: race TSS unknown; TSB shown assumes zero race load"
            ));
        }

        ctl = update_load(ctl, combined.tss, 42.0);
        atl = update_load(atl, combined.tss, 7.0);
        let tsb = round_to_2(ctl - atl);

        days.push(ForwardLoadDay {
            date: date_key,
            planned_tss: round_to_2(combined.tss),
            planned_duration_seconds: combined.duration_seconds,
            source: combined.source,
            tss_source: combined.tss_source,
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
        notes,
    })
    .to_string()
}
