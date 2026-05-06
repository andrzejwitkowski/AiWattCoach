use std::{future::Future, pin::Pin};

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use crate::domain::{
    completed_workouts::compute_power_curve,
    llm::LlmToolDefinition,
    llm_tools::{LlmTool, ToolExecutionContext},
};

mod response;
use response::build_power_curve_response;

const SELECTED_WORKOUT_POWER_CURVE_TOOL_NAME: &str = "selected_workout_power_curve";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerCurveArgs {
    date: String,
    #[serde(default)]
    workout_id: Option<String>,
    #[serde(default = "default_resolution")]
    resolution_seconds: u16,
}

fn default_resolution() -> u16 {
    5
}

/// LLM-callable tool that returns the power curve (mean-max average watts) for a
/// selected completed workout.
///
/// At 5-second resolution the tool prefers a persisted cache; if none exists it
/// computes the curve and stores it. Other resolutions are computed on-the-fly
/// and never persisted.
pub struct SelectedWorkoutPowerCurve;

impl LlmTool for SelectedWorkoutPowerCurve {
    fn name(&self) -> &'static str {
        SELECTED_WORKOUT_POWER_CURVE_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Get the power curve (mean-max average watts) for a selected completed workout. Returns average power for successive durations. Only available for completed workouts with power data.".to_string(),
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
                    "resolution_seconds": {
                        "type": "integer",
                        "description": "Duration resolution in seconds. Must be a multiple of 5. Default 5.",
                        "default": 5
                    }
                },
                "required": ["date"]
            })
            .to_string(),
        }
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let ctx = context.clone();
        Box::pin(async move { execute_power_curve(&args, &ctx).await })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: PowerCurveArgs = serde_json::from_str(arguments_json).ok()?;
        parse_date(&args.date)?;
        let mut out = format!("date {} resolution {}s", args.date, args.resolution_seconds);
        if let Some(ref id) = args.workout_id {
            out.push_str(&format!(" workout {id}"));
        }
        Some(out)
    }

    fn is_available(&self, context: &ToolExecutionContext) -> bool {
        context.data_port.is_some()
    }
}

async fn execute_power_curve(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let args = match parse_args(arguments_json) {
        Ok(args) => args,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    let resolution = args.resolution_seconds;
    if resolution < 5 || !resolution.is_multiple_of(5) {
        return json!({
            "error": "invalid resolution",
            "reason": "resolution_seconds must be a multiple of 5, minimum 5"
        })
        .to_string();
    }

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
        return build_unavailable_response(
            &args.date,
            &workout.completed_workout_id,
            workout.name.as_deref(),
            "completed workout details are unavailable",
            workout.details_unavailable_reason.as_deref(),
        );
    }

    if resolution == 5 {
        if let Some(cached) = &workout.power_curve_5s {
            return build_power_curve_response(
                &args.date,
                &workout.completed_workout_id,
                workout.name.as_deref(),
                cached,
                "stored_5s",
            );
        }

        match compute_power_curve(&workout, 5) {
            Ok(curve) => {
                let source = match port
                    .persist_power_curve_5s_if_missing(
                        &context.user_id,
                        &workout.completed_workout_id,
                        curve.clone(),
                    )
                    .await
                {
                    Ok(()) => "computed_and_persisted_5s",
                    Err(err) => {
                        tracing::warn!(
                            user_id = %context.user_id,
                            completed_workout_id = %workout.completed_workout_id,
                            error = %err,
                            "failed to persist 5s power curve"
                        );
                        "computed_5s_not_persisted"
                    }
                };
                return build_power_curve_response(
                    &args.date,
                    &workout.completed_workout_id,
                    workout.name.as_deref(),
                    &curve,
                    source,
                );
            }
            Err(err) => {
                return build_unavailable_response(
                    &args.date,
                    &workout.completed_workout_id,
                    workout.name.as_deref(),
                    &power_curve_error_reason(&err),
                    None,
                );
            }
        }
    }

    match compute_power_curve(&workout, resolution) {
        Ok(curve) => build_power_curve_response(
            &args.date,
            &workout.completed_workout_id,
            workout.name.as_deref(),
            &curve,
            "computed_ad_hoc",
        ),
        Err(err) => build_unavailable_response(
            &args.date,
            &workout.completed_workout_id,
            workout.name.as_deref(),
            &power_curve_error_reason(&err),
            None,
        ),
    }
}

fn select_workout(
    args: &PowerCurveArgs,
    completed: Vec<crate::domain::completed_workouts::CompletedWorkout>,
) -> Result<crate::domain::completed_workouts::CompletedWorkout, String> {
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

fn power_curve_error_reason(err: &crate::domain::completed_workouts::PowerCurveError) -> String {
    match err {
        crate::domain::completed_workouts::PowerCurveError::InvalidResolution => {
            "invalid resolution".to_string()
        }
        crate::domain::completed_workouts::PowerCurveError::DetailsUnavailable => {
            "workout details are unavailable".to_string()
        }
        crate::domain::completed_workouts::PowerCurveError::WattsStreamMissing => {
            "no watts power stream in workout details".to_string()
        }
        crate::domain::completed_workouts::PowerCurveError::WattsStreamUnsupportedType => {
            "watts stream uses an unsupported series type".to_string()
        }
        crate::domain::completed_workouts::PowerCurveError::NoValidPowerSamples => {
            "no valid power samples available".to_string()
        }
        crate::domain::completed_workouts::PowerCurveError::InsufficientData => {
            "not enough data for the requested resolution".to_string()
        }
    }
}

fn build_unavailable_response(
    date: &str,
    workout_id: &str,
    workout_name: Option<&str>,
    reason: &str,
    details_reason: Option<&str>,
) -> String {
    let mut resp = json!({
        "date": date,
        "workout_id": workout_id,
        "error": "power curve unavailable",
        "reason": reason,
    });
    if let Some(name) = workout_name {
        resp["workout_name"] = json!(name);
    }
    if let Some(r) = details_reason {
        resp["details_unavailable_reason"] = json!(r);
    }
    resp.to_string()
}

fn parse_args(arguments_json: &str) -> Result<PowerCurveArgs, String> {
    let args: PowerCurveArgs =
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

#[cfg(test)]
mod tests;
