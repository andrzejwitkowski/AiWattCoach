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
    training_context::{
        PlannedWorkoutContext, ProjectedWorkoutContext, RecentDayContext, UpcomingDayContext,
    },
    workout_summary::WorkoutSummary,
};

mod port;
mod response;

pub use port::GetSelectedWorkoutDataPort;
use response::{
    build_selected_workout_response, SelectedDate, SelectedPlannedWorkout, SelectedWorkoutData,
};

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
            description: "Get detailed workout data for a specific date. Returns completed workouts with full statistics, capped raw power/cadence/heart-rate streams, and AI conversation history. Also returns planned workouts (if future or not completed) and basic race info if no completed workout exists for that day.".to_string(),
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
            "use for questions about a specific date when you need detailed completed, planned, race, or workout-summary data instead of relying only on packed context",
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

    let data = match load_selected_workout_data(&date, context, port.as_ref()).await {
        Ok(data) => data,
        Err(err) => return json!({ "error": err }).to_string(),
    };

    let response_date = date.value.clone();
    json!(build_selected_workout_response(
        response_date,
        data,
        &date,
        &context.today,
    ))
    .to_string()
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

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

async fn load_selected_workout_data(
    date: &SelectedDate,
    context: &ToolExecutionContext,
    port: &dyn GetSelectedWorkoutDataPort,
) -> Result<SelectedWorkoutData, String> {
    let user_id = &context.user_id;
    let completed = load_completed(port, user_id, &date.value).await?;
    let planned = load_planned(port, user_id, &date.value)
        .await?
        .into_iter()
        .map(SelectedPlannedWorkout::from_planned_workout)
        .collect::<Vec<_>>();
    let planned = if planned.is_empty() && completed.is_empty() {
        load_planned_from_training_context(date, context)
    } else {
        planned
    };
    let races = load_races(port, user_id, &date.value).await?;
    let summaries = load_summaries(port, user_id, &completed).await?;

    Ok(SelectedWorkoutData {
        completed,
        planned,
        races,
        summaries,
    })
}

fn load_planned_from_training_context(
    date: &SelectedDate,
    context: &ToolExecutionContext,
) -> Vec<SelectedPlannedWorkout> {
    let mut planned = context
        .training_context
        .recent_days
        .iter()
        .filter(|day| day.date == date.value)
        .flat_map(selected_planned_from_recent_day)
        .collect::<Vec<_>>();

    planned.extend(
        context
            .training_context
            .upcoming_days
            .iter()
            .filter(|day| day.date == date.value)
            .flat_map(selected_planned_from_upcoming_day),
    );

    planned.extend(
        context
            .training_context
            .projected_days
            .iter()
            .filter(|day| day.date == date.value)
            .flat_map(selected_planned_from_projected_day),
    );

    planned
}

fn selected_planned_from_recent_day(day: &RecentDayContext) -> Vec<SelectedPlannedWorkout> {
    day.planned_workouts
        .iter()
        .map(selected_planned_from_planned_context)
        .collect()
}

fn selected_planned_from_upcoming_day(day: &UpcomingDayContext) -> Vec<SelectedPlannedWorkout> {
    day.planned_workouts
        .iter()
        .map(selected_planned_from_planned_context)
        .collect()
}

fn selected_planned_from_projected_day(
    day: &crate::domain::training_context::ProjectedDayContext,
) -> Vec<SelectedPlannedWorkout> {
    day.workouts
        .iter()
        .enumerate()
        .map(|(index, workout)| {
            selected_planned_from_projected_context(day.date.as_str(), workout, index)
        })
        .collect()
}

fn selected_planned_from_planned_context(
    workout: &PlannedWorkoutContext,
) -> SelectedPlannedWorkout {
    SelectedPlannedWorkout {
        planned_workout_id: format!("context-event:{}", workout.event_id),
        date: workout
            .start_date_local
            .get(..10)
            .unwrap_or(&workout.start_date_local)
            .to_string(),
        name: workout.name.clone(),
        rest_day: false,
        rest_day_reason: None,
        raw_workout_doc: workout.raw_workout_doc.clone(),
    }
}

fn selected_planned_from_projected_context(
    date: &str,
    workout: &ProjectedWorkoutContext,
    index: usize,
) -> SelectedPlannedWorkout {
    SelectedPlannedWorkout {
        planned_workout_id: format!("projected:{}:{date}:{index}", workout.source_workout_id),
        date: date.to_string(),
        name: workout.name.clone(),
        rest_day: workout.rest_day,
        rest_day_reason: workout.rest_day_reason.clone(),
        raw_workout_doc: workout.raw_workout_doc.clone(),
    }
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
