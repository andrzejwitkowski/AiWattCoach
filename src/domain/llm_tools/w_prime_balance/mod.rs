use std::{future::Future, pin::Pin};

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutSeries},
    llm::LlmToolDefinition,
    llm_tools::{LlmTool, ToolExecutionContext},
};

mod response;
use response::{build_w_prime_balance_response, WPrimeBalanceOutput};

#[cfg(test)]
mod tests;

const W_PRIME_BALANCE_TOOL_NAME: &str = "get_w_prime_balance";
const DEFAULT_CP_WATTS: i32 = 250;
const DEFAULT_W_PRIME_JOULES: i32 = 20_000;
const CP_FTP_RATIO: f64 = 0.90;
const W_PRIME_PER_KG: f64 = 280.0;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct WPrimeBalanceArgs {
    date: String,
    #[serde(default)]
    workout_id: Option<String>,
    #[serde(default)]
    cp_watts: Option<i32>,
    #[serde(default)]
    w_prime_joules: Option<i32>,
}

pub struct WPrimeBalance;

impl LlmTool for WPrimeBalance {
    fn name(&self) -> &'static str {
        W_PRIME_BALANCE_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Compute W' (W-prime) balance for a completed workout. Tracks anaerobic work capacity depletion and recovery for each second using the Skiba differential model. When power exceeds Critical Power, W' depletes linearly; when power drops below CP, W' recovers exponentially. Returns time-series balance data and summary statistics including time spent at various depletion levels. Only available for completed workouts with power data.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "date": {
                        "type": "string",
                        "description": "Date in YYYY-MM-DD format, e.g. 2026-05-05"
                    },
                    "workout_id": {
                        "type": "string",
                        "description": "Completed workout ID for the date. Required when multiple completed workouts exist on the same day."
                    },
                    "cp_watts": {
                        "type": "integer",
                        "description": "Critical Power in watts. If omitted, estimated as 90% of athlete FTP from context."
                    },
                    "w_prime_joules": {
                        "type": "integer",
                        "description": "W' capacity in joules. If omitted, estimated from athlete body weight (280 J/kg) from context."
                    }
                },
                "required": ["date"]
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "very useful for post-race analysis — use after selecting a completed workout to evaluate anaerobic capacity depletion, pacing strategy, or interval execution quality; prefer this over guessing from normalized power or intensity factor alone",
        )
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let ctx = context.clone();
        Box::pin(async move { execute_w_prime_balance(&args, &ctx).await })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: WPrimeBalanceArgs = serde_json::from_str(arguments_json).ok()?;
        parse_date(&args.date)?;
        let mut out = format!("date {}", args.date);
        if let Some(ref id) = args.workout_id {
            out.push_str(&format!(" workout {id}"));
        }
        if let Some(cp) = args.cp_watts {
            out.push_str(&format!(" CP={cp}W"));
        }
        if let Some(wp) = args.w_prime_joules {
            out.push_str(&format!(" W'={wp}J"));
        }
        Some(out)
    }

    fn is_available(&self, context: &ToolExecutionContext) -> bool {
        context.data_port.is_some()
    }
}

async fn execute_w_prime_balance(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let args = match parse_args(arguments_json) {
        Ok(args) => args,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    let Some(port) = context.data_port.as_ref() else {
        return json!({ "error": "data port not available" }).to_string();
    };

    let completed = match port
        .list_completed_by_date_range(&context.user_id, &args.date, &args.date)
        .await
    {
        Ok(workouts) => workouts,
        Err(err) => {
            return json!({ "error": format!("failed to load completed workouts: {err}") })
                .to_string()
        }
    };

    let workout = match select_workout(&args, completed) {
        Ok(w) => w,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    if workout.details_unavailable_reason.is_some() {
        return json!({
            "error": "W' balance computation skipped",
            "reason": "completed workout details are unavailable",
            "date": args.date,
            "workout_id": workout.completed_workout_id,
        })
        .to_string();
    }

    let power_samples = match extract_power_samples(&workout) {
        Ok(samples) => samples,
        Err(reason) => {
            return json!({
                "error": "W' balance computation skipped",
                "reason": reason,
                "date": args.date,
                "workout_id": workout.completed_workout_id,
            })
            .to_string();
        }
    };

    let valid_power_samples = power_samples.iter().filter(|v| v.is_some()).count();
    if valid_power_samples == 0 {
        return json!({
            "error": "W' balance computation skipped",
            "reason": "no valid power samples available",
            "date": args.date,
            "workout_id": workout.completed_workout_id,
        })
        .to_string();
    }

    let (cp_watts, cp_source) = estimate_cp(&args, context);
    let (w_prime_joules, w_prime_source) = estimate_w_prime(&args, context);

    let (
        balance_series,
        start_balance,
        end_balance,
        min_balance,
        max_deficit,
        time_above_90,
        time_50_to_90,
        time_10_to_50,
        time_below_10,
        depleted,
    ) = compute_w_prime_balance(&power_samples, cp_watts, w_prime_joules);

    let sample_count = power_samples.len();

    let output = WPrimeBalanceOutput {
        date: args.date,
        workout_id: workout.completed_workout_id,
        workout_name: workout.name,
        cp_watts,
        w_prime_joules,
        cp_source,
        w_prime_source,
        sample_count,
        valid_power_samples,
        balance_series,
        start_balance,
        end_balance,
        min_balance,
        max_deficit,
        time_above_90,
        time_50_to_90,
        time_10_to_50,
        time_below_10,
        depleted,
    };

    build_w_prime_balance_response(&output)
}

fn parse_args(arguments_json: &str) -> Result<WPrimeBalanceArgs, String> {
    let args: WPrimeBalanceArgs =
        serde_json::from_str(arguments_json).map_err(|err| format!("invalid arguments: {err}"))?;
    if parse_date(&args.date).is_none() {
        return Err(format!(
            "invalid date: expected YYYY-MM-DD, got {}",
            args.date
        ));
    }
    Ok(args)
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn select_workout(
    args: &WPrimeBalanceArgs,
    completed: Vec<CompletedWorkout>,
) -> Result<CompletedWorkout, String> {
    if completed.is_empty() {
        return Err(format!(
            "no completed workouts found for date {}",
            args.date
        ));
    }

    if let Some(ref workout_id) = args.workout_id {
        completed
            .into_iter()
            .find(|w| w.completed_workout_id == *workout_id)
            .ok_or_else(|| {
                format!(
                    "completed workout {workout_id} not found for date {}",
                    args.date
                )
            })
    } else if completed.len() == 1 {
        Ok(completed.into_iter().next().unwrap())
    } else {
        let ids: Vec<String> = completed
            .iter()
            .map(|w| w.completed_workout_id.clone())
            .collect();
        Err(format!(
            "multiple completed workouts found for date {}. Provide workout_id. Candidates: {ids:?}",
            args.date
        ))
    }
}

fn extract_power_samples(workout: &CompletedWorkout) -> Result<Vec<Option<i32>>, String> {
    let watts_stream = workout
        .details
        .streams
        .iter()
        .find(|s| s.stream_type.to_lowercase() == "watts")
        .ok_or("no watts power stream in workout details")?;

    let series = watts_stream
        .primary_series
        .as_ref()
        .or(watts_stream.secondary_series.as_ref());

    match series {
        Some(CompletedWorkoutSeries::Integers(values)) => Ok(values
            .iter()
            .map(|&v| (v >= 0).then_some(v as i32))
            .collect()),
        Some(CompletedWorkoutSeries::Floats(values)) => Ok(values
            .iter()
            .map(|&v| {
                if v.is_finite() && v >= 0.0 && v <= i32::MAX as f64 {
                    Some(v.round() as i32)
                } else {
                    None
                }
            })
            .collect()),
        _ => Err("watts stream uses an unsupported series type".to_string()),
    }
}

fn estimate_cp(args: &WPrimeBalanceArgs, context: &ToolExecutionContext) -> (i32, String) {
    if let Some(cp) = args.cp_watts {
        return (cp, "user_provided".to_string());
    }
    if let Some(ftp) = context.training_context.history.ftp_current {
        if ftp > 0 {
            let cp = (ftp as f64 * CP_FTP_RATIO).floor() as i32;
            return (cp, "estimated_from_ftp".to_string());
        }
    }
    (DEFAULT_CP_WATTS, "default".to_string())
}

fn estimate_w_prime(args: &WPrimeBalanceArgs, context: &ToolExecutionContext) -> (i32, String) {
    if let Some(wp) = args.w_prime_joules {
        return (wp, "user_provided".to_string());
    }
    if let Some(weight) = context.training_context.profile.weight_kg {
        if weight > 0.0 {
            let wp = (weight * W_PRIME_PER_KG).floor() as i32;
            return (wp, "estimated_from_weight".to_string());
        }
    }
    (DEFAULT_W_PRIME_JOULES, "default".to_string())
}

#[allow(clippy::too_many_arguments)]
fn compute_w_prime_balance(
    power_samples: &[Option<i32>],
    cp_watts: i32,
    w_prime_joules: i32,
) -> (Vec<f64>, f64, f64, f64, f64, u32, u32, u32, u32, bool) {
    let cp = cp_watts as f64;
    let w_prime = w_prime_joules as f64;
    let mut balance = w_prime;
    let mut series = Vec::with_capacity(power_samples.len());
    let mut min_balance = w_prime;
    let mut max_deficit = 0.0f64;
    let mut time_above_90 = 0u32;
    let mut time_50_to_90 = 0u32;
    let mut time_10_to_50 = 0u32;
    let mut time_below_10 = 0u32;
    let mut depleted = false;

    for sample in power_samples {
        if let Some(power) = sample {
            let p = *power as f64;
            if p > cp {
                balance -= p - cp;
                if balance <= 0.0 {
                    balance = 0.0;
                    depleted = true;
                }
            } else if balance < w_prime {
                let tau = w_prime / (cp - p);
                balance = w_prime - (w_prime - balance) * (-1.0 / tau).exp();
            }

            if balance < min_balance {
                min_balance = balance;
            }
            let deficit = w_prime - balance;
            if deficit > max_deficit {
                max_deficit = deficit;
            }

            let pct = balance / w_prime;
            if pct >= 0.9 {
                time_above_90 += 1;
            } else if pct >= 0.5 {
                time_50_to_90 += 1;
            } else if pct >= 0.1 {
                time_10_to_50 += 1;
            } else {
                time_below_10 += 1;
            }
        }

        series.push(balance);
    }

    let start_balance = w_prime;
    let end_balance = *series.last().unwrap_or(&w_prime);

    (
        series,
        start_balance,
        end_balance,
        min_balance,
        max_deficit,
        time_above_90,
        time_50_to_90,
        time_10_to_50,
        time_below_10,
        depleted,
    )
}
