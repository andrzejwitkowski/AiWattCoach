use std::{future::Future, pin::Pin};

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use crate::domain::{
    completed_workouts::CompletedWorkout,
    llm::LlmToolDefinition,
    llm_tools::{LlmTool, ToolExecutionContext},
    planned_workouts::PlannedWorkout,
    races::Race,
    workout_summary::WorkoutSummary,
};

mod port;
mod response;

pub use port::GetSelectedWorkoutDataPort;
use response::build_selected_workout_response;
pub(crate) use response::SelectedDate;
pub use response::SelectedWorkoutData;

const GET_SELECTED_WORKOUT_TOOL_NAME: &str = "get_selected_workout";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetSelectedWorkoutArgs {
    date: String,
}

/// Get detailed workout data for a specific date.
pub struct GetSelectedWorkout;

impl LlmTool for GetSelectedWorkout {
    fn name(&self) -> &'static str {
        GET_SELECTED_WORKOUT_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Get detailed workout data for a specific date. Returns completed workouts with statistics and aligned_intervals when a linked planned workout exists (primary plan-vs-actual evidence). Heartrate stream at 1s resolution when present. Also returns planned workouts (if future or not completed) and basic race info if no completed workout exists for that day.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "date": {
                        "type": "string",
                        "description": "Date in YYYY-MM-DD format, e.g. 2026-05-05"
                    }
                },
                "required": ["date"]
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "use when aligned_intervals in context are insufficient or you need heartrate streams; returns aligned_intervals for linked planned workouts",
        )
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let ctx = context.clone();
        Box::pin(async move { get_selected_workout(&args, &ctx).await })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: GetSelectedWorkoutArgs = serde_json::from_str(arguments_json).ok()?;
        parse_date(&args.date)?;
        Some(format!("date {}", args.date))
    }

    fn is_available(&self, context: &ToolExecutionContext) -> bool {
        context.data_port.is_some()
    }
}

async fn get_selected_workout(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let date = match parse_args(arguments_json) {
        Ok(date) => date,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    let Some(port) = context.data_port.as_ref() else {
        return json!({ "error": "data port not available" }).to_string();
    };

    let data = match load_selected_workout_data_for_date(&date, context, port.as_ref()).await {
        Ok(data) => data,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    build_response_for_date(date, data, &context.today)
}

fn parse_args(arguments_json: &str) -> Result<SelectedDate, String> {
    let args = serde_json::from_str::<GetSelectedWorkoutArgs>(arguments_json)
        .map_err(|err| format!("invalid arguments: {err}"))?;
    let parsed = parse_date(&args.date)
        .ok_or_else(|| format!("invalid date: expected YYYY-MM-DD, got {}", args.date))?;

    Ok(SelectedDate {
        value: args.date,
        parsed,
    })
}

pub(crate) fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub(crate) fn build_response_for_date(
    date: SelectedDate,
    data: SelectedWorkoutData,
    today: &str,
) -> String {
    let response_date = date.value.clone();
    json!(build_selected_workout_response(
        response_date,
        data,
        &date,
        today,
    ))
    .to_string()
}

pub(crate) async fn load_selected_workout_data_for_date(
    date: &SelectedDate,
    context: &ToolExecutionContext,
    port: &dyn GetSelectedWorkoutDataPort,
) -> Result<SelectedWorkoutData, String> {
    let user_id = &context.user_id;
    let completed = load_completed(port, user_id, &date.value).await?;
    let planned = load_planned(port, user_id, &date.value).await?;
    let races = load_races(port, user_id, &date.value).await?;
    let summaries = load_summaries(port, user_id, &completed).await?;

    Ok(SelectedWorkoutData {
        completed,
        planned,
        races,
        summaries,
    })
}

async fn load_completed(
    port: &dyn GetSelectedWorkoutDataPort,
    user_id: &str,
    date: &str,
) -> Result<Vec<CompletedWorkout>, String> {
    port.list_completed_by_date_range(user_id, date, date)
        .await
        .map_err(|err| format!("failed to load completed workouts: {err}"))
}

async fn load_planned(
    port: &dyn GetSelectedWorkoutDataPort,
    user_id: &str,
    date: &str,
) -> Result<Vec<PlannedWorkout>, String> {
    port.list_planned_by_date_range(user_id, date, date)
        .await
        .map_err(|err| format!("failed to load planned workouts: {err}"))
}

async fn load_races(
    port: &dyn GetSelectedWorkoutDataPort,
    user_id: &str,
    date: &str,
) -> Result<Vec<Race>, String> {
    port.list_races_by_date_range(user_id, date, date)
        .await
        .map_err(|err| format!("failed to load races: {err}"))
}

async fn load_summaries(
    port: &dyn GetSelectedWorkoutDataPort,
    user_id: &str,
    completed: &[CompletedWorkout],
) -> Result<Vec<WorkoutSummary>, String> {
    let completed_ids: Vec<String> = completed
        .iter()
        .map(|w| w.completed_workout_id.clone())
        .collect();

    if completed_ids.is_empty() {
        return Ok(Vec::new());
    }

    port.find_summaries_by_workout_ids(user_id, completed_ids)
        .await
        .map_err(|err| format!("failed to load workout summaries: {err}"))
}

#[cfg(test)]
mod tests;
