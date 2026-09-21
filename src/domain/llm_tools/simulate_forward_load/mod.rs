use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{
    intervals::{parse_planned_workout_days, PlannedWorkoutDay, PlannedWorkoutLine},
    llm::LlmToolDefinition,
    training_context::{FuturePlannedEventContext, RaceContext},
};

use super::{LlmTool, ToolExecutionContext};

mod estimates;
#[cfg(test)]
mod tests;

pub(crate) use estimates::duration_from_distance_meters;

use estimates::{
    combine_estimates, format_date, input_day_estimate, parse_date, round_to_2,
    select_estimates_for_day, snapshot_baseline, update_load, Baseline, LoadSources, TssSource,
};

const SIMULATE_FORWARD_LOAD_TOOL_NAME: &str = "simulate_forward_load";
const DEFAULT_FORECAST_DAYS: u32 = 14;
const MAX_FORECAST_DAYS: u32 = 45;

#[derive(Deserialize)]
struct SimulateForwardLoadArgs {
    dated_workout_text: Option<String>,
    horizon_days: Option<u32>,
    completed_race: Option<CompletedRaceArg>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CompletedRaceSource {
    Measured,
    Estimated,
}

#[derive(Deserialize)]
struct CompletedRaceArg {
    date: String,
    tss: f64,
    source: CompletedRaceSource,
}

#[derive(Serialize)]
struct SimulateForwardLoadResponse {
    baseline: ForwardLoadBaseline,
    days: Vec<ForwardLoadDay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_applied_load: Option<BaselineAppliedLoad>,
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
struct BaselineAppliedLoad {
    date: String,
    tss: f64,
    tss_source: TssSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<CompletedRaceSource>,
}

#[derive(Serialize)]
struct ForwardLoadDay {
    date: String,
    planned_tss: f64,
    planned_duration_seconds: Option<i32>,
    source: LoadSources,
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
            description: format!(
                "Simulate forward training load from today for a configurable horizon (default {DEFAULT_FORECAST_DAYS} days, max {MAX_FORECAST_DAYS}). Set horizon_days to reach the athlete's next A-priority event. The tool automatically includes already-scheduled workouts (upcoming days), projected workouts, and future events (races). Only provide dated_workout_text for days you want to override or add new workouts inside the projection window.\n\nPass a finished race as completed_race with the recap TSS (measured or estimated). Do not encode completed races in dated_workout_text — that path is for draft overrides only and rejects invalid workout syntax.\n\nEach day includes tss_source (planned|projected|default|estimated|none). Race/event load with measured TSS uses planned; when only duration/distance is known, TSS is estimated (hours×IF²×100) and labeled estimated; when neither exists, tss_source is none and notes explain zero race load.\n\nFormat: Each day starts with a YYYY-MM-DD header on its own line, followed by workout steps or 'Rest Day'. You can use section titles, ramps, repeat headers (Nx), and power targets in %FTP or watts.\n\nExample 1 - Simple:\n2026-05-05\n- 90m 65%\n2026-05-06\nRest Day: recovery\n\nExample 2 - Complex interval session:\n2026-05-07\nWarmup\n- 15m ramp 55-75%\n\nMain Set\n4x\n- 2m 105%\n- 1m 65%\n\n3x\n- 3m 95%\n- 2m 65%\n\nCooldown\n- 10m 55%"
            ),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dated_workout_text": {
                        "type": "string",
                        "description": "Optional. Dated workout text for in-window draft overrides or additions, in YYYY-MM-DD plus workout-builder format. If omitted, the simulation uses only existing scheduled workouts, projections, and events from context. Dates outside 1..horizon are noted as ignored. Do not use this for completed races — use completed_race."
                    },
                    "horizon_days": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_FORECAST_DAYS,
                        "description": "Days of forward projection; set this to reach the athlete's next A-priority event. Defaults to 14; clamped to 1..=45."
                    },
                    "completed_race": {
                        "type": "object",
                        "additionalProperties": false,
                        "description": "Optional. Finished race load applied to the baseline regardless of whether its date falls in 1..horizon. Use the recap TSS; source is measured when the file reports TSS, otherwise estimated.",
                        "required": ["date", "tss", "source"],
                        "properties": {
                            "date": {
                                "type": "string",
                                "description": "Race date YYYY-MM-DD (today, past, or the day before the window)."
                            },
                            "tss": {
                                "type": "number",
                                "description": "Training stress score from the workout recap."
                            },
                            "source": {
                                "type": "string",
                                "enum": ["measured", "estimated"],
                                "description": "measured when TSS comes from the completed file; estimated when derived."
                            }
                        }
                    }
                }
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "use when reasoning about future fatigue, load progression, or the impact of planned workouts; prefer this over mental arithmetic from CTL/ATL/TSB alone; set horizon_days when the next A-event is beyond the default 14-day window",
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
        let mut parts = Vec::new();
        if let Some(race) = args.completed_race.as_ref() {
            parts.push(format!(
                "completed_race {} tss={}",
                race.date,
                round_to_2(race.tss)
            ));
        }
        if let Some(text) = args.dated_workout_text.as_deref() {
            if let Ok(parsed) = parse_planned_workout_days(text) {
                if let (Some(first), Some(last)) = (parsed.days.first(), parsed.days.last()) {
                    let count = parsed.days.len();
                    let day_word = if count == 1 { "day" } else { "days" };
                    parts.push(format!(
                        "{count} dated {day_word} from {} to {}",
                        first.date, last.date
                    ));
                }
            }
        }
        if let Some(days) = args.horizon_days {
            parts.push(format!("horizon_days={days}"));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
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

    let has_completed_race = args.completed_race.is_some();
    let mut notes = Vec::new();
    let input_days = match args.dated_workout_text.as_deref() {
        None => Vec::new(),
        Some(text) if text.trim().is_empty() => {
            tracing::info!(
                empty_dated_workout_text_normalized = true,
                "simulate_forward_load normalized empty dated_workout_text"
            );
            Vec::new()
        }
        Some(text) => match parse_planned_workout_days(text) {
            Ok(parsed) => parsed.days,
            Err(error) if has_completed_race => {
                notes.push(format!(
                    "dated_workout_text ignored (invalid syntax with completed_race present): {error}"
                ));
                Vec::new()
            }
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

    let horizon = args
        .horizon_days
        .unwrap_or(DEFAULT_FORECAST_DAYS)
        .clamp(1, MAX_FORECAST_DAYS);
    let ftp_watts = context.training_context.history.ftp_current;
    let baseline = snapshot_baseline(&context.training_context);
    let (starting, completed_applied) = match args.completed_race.as_ref() {
        Some(race) => {
            let applied = apply_completed_race_load(race, baseline);
            (
                Baseline {
                    ctl: applied.ctl,
                    atl: applied.atl,
                    tsb: applied.ctl - applied.atl,
                },
                applied.applied,
            )
        }
        None => (baseline, None),
    };
    let OutOfWindowLoad {
        mut ctl,
        mut atl,
        applied: dated_applied,
    } = apply_out_of_window_input_load(
        &input_days,
        today,
        horizon,
        ftp_watts,
        starting,
        &mut notes,
        completed_applied.is_some(),
    );
    let baseline_applied_load = completed_applied.or(dated_applied);

    let baseline_out = ForwardLoadBaseline {
        today: context.today.clone(),
        ctl: round_to_2(ctl),
        atl: round_to_2(atl),
        tsb: if baseline_applied_load.is_some() {
            round_to_2(ctl - atl)
        } else {
            round_to_2(baseline.tsb)
        },
        ftp_watts,
    };

    let mut days = Vec::with_capacity(horizon as usize);
    for offset in 1..=horizon {
        let date = today + Duration::days(i64::from(offset));
        let date_key = format_date(date);

        let estimates = select_estimates_for_day(
            &input_days,
            &context.training_context.upcoming_days,
            &context.training_context.projected_days,
            &context.training_context.future_events,
            &context.training_context.races,
            &date_key,
            ftp_watts,
        );

        let combined = combine_estimates(estimates);
        if combined.emit_race_tss_unknown_note {
            notes.push(format!(
                "{date_key}: race TSS unknown; TSB shown assumes zero race load"
            ));
        } else if combined.tss_source == TssSource::Estimated {
            notes.push(format!(
                "{date_key}: race TSS estimated (tss={:.2}, duration_s={}); not measured",
                combined.tss,
                combined
                    .duration_seconds
                    .map(|seconds| seconds.to_string())
                    .unwrap_or_else(|| "n/a".to_string())
            ));
        }

        ctl = update_load(ctl, combined.tss, 42.0);
        atl = update_load(atl, combined.tss, 7.0);
        let tsb = round_to_2(ctl - atl);

        days.push(ForwardLoadDay {
            date: date_key,
            planned_tss: round_to_2(combined.tss),
            planned_duration_seconds: combined.duration_seconds,
            source: combined.sources,
            tss_source: combined.tss_source,
            rest_day: combined.is_rest,
            rest_day_reason: combined.rest_reason,
            ctl: round_to_2(ctl),
            atl: round_to_2(atl),
            tsb,
        });
    }

    push_events_beyond_horizon_notes(
        &mut notes,
        &context.training_context.races,
        &context.training_context.future_events,
        today,
        horizon,
    );

    json!(SimulateForwardLoadResponse {
        baseline: baseline_out,
        days,
        baseline_applied_load,
        notes,
    })
    .to_string()
}

struct OutOfWindowLoad {
    ctl: f64,
    atl: f64,
    applied: Option<BaselineAppliedLoad>,
}

fn apply_completed_race_load(race: &CompletedRaceArg, baseline: Baseline) -> OutOfWindowLoad {
    let ctl = update_load(baseline.ctl, race.tss, 42.0);
    let atl = update_load(baseline.atl, race.tss, 7.0);
    let tss_source = match race.source {
        CompletedRaceSource::Measured => TssSource::Planned,
        CompletedRaceSource::Estimated => TssSource::Estimated,
    };
    OutOfWindowLoad {
        ctl,
        atl,
        applied: Some(BaselineAppliedLoad {
            date: race.date.clone(),
            tss: round_to_2(race.tss),
            tss_source,
            source: Some(race.source),
        }),
    }
}

fn apply_out_of_window_input_load(
    input_days: &[PlannedWorkoutDay],
    today: NaiveDate,
    horizon: u32,
    ftp_watts: Option<i32>,
    baseline: Baseline,
    notes: &mut Vec<String>,
    skip_dated_race_apply: bool,
) -> OutOfWindowLoad {
    let last_projected = today + Duration::days(i64::from(horizon));
    let mut ctl = baseline.ctl;
    let mut atl = baseline.atl;
    let mut applied = None;

    for day in input_days {
        let Some(day_date) = parse_date(&day.date) else {
            notes.push(format!(
                "{}: dated workout ignored (invalid date, outside 1..{horizon} projection window)",
                day.date
            ));
            continue;
        };
        if day_date > today && day_date <= last_projected {
            continue;
        }

        let title = input_day_title(day);
        if !skip_dated_race_apply
            && applied.is_none()
            && day_date <= today
            && !day.is_rest_day()
            && is_baseline_race_override_name(&title)
        {
            if let Some(estimate) =
                input_day_estimate(std::slice::from_ref(day), &day.date, ftp_watts)
            {
                let tss_source = if title.to_ascii_lowercase().contains("estimat") {
                    TssSource::Estimated
                } else {
                    estimate.tss_source
                };
                ctl = update_load(ctl, estimate.tss, 42.0);
                atl = update_load(atl, estimate.tss, 7.0);
                applied = Some(BaselineAppliedLoad {
                    date: day.date.clone(),
                    tss: round_to_2(estimate.tss),
                    tss_source,
                    source: None,
                });
                continue;
            }
        }

        let reason = if day_date == today {
            "today"
        } else if day_date < today {
            "past"
        } else {
            "beyond horizon"
        };
        notes.push(format!(
            "{}: dated workout ignored ({reason}, outside 1..{horizon} projection window)",
            day.date
        ));
    }

    OutOfWindowLoad { ctl, atl, applied }
}

fn push_events_beyond_horizon_notes(
    notes: &mut Vec<String>,
    races: &[RaceContext],
    future_events: &[FuturePlannedEventContext],
    today: NaiveDate,
    horizon: u32,
) {
    let last = today + Duration::days(i64::from(horizon));
    for race in races {
        if race.priority.eq_ignore_ascii_case("A") {
            push_outside_horizon_event_note(notes, &race.date, last, horizon);
        }
    }
    for event in future_events {
        if let Some(date_str) = event.start_date_local.get(..10) {
            push_outside_horizon_event_note(notes, date_str, last, horizon);
        }
    }
}

fn push_outside_horizon_event_note(
    notes: &mut Vec<String>,
    date_str: &str,
    last: NaiveDate,
    horizon: u32,
) {
    let Some(date) = parse_date(date_str) else {
        return;
    };
    if date > last {
        notes.push(format!(
            "{date_str}: A-priority event outside projection horizon (horizon_days={horizon})"
        ));
    }
}

fn input_day_title(day: &PlannedWorkoutDay) -> String {
    day.planned_workout()
        .and_then(|workout| {
            workout.lines.iter().find_map(|line| match line {
                PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

fn is_baseline_race_override_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.contains("race") {
        return false;
    }
    if lower.contains("pre-race") || lower.contains("race opener") || lower.contains("race pace") {
        return false;
    }
    true
}
