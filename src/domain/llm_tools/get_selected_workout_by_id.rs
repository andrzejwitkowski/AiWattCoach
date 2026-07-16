use std::{future::Future, pin::Pin};

use serde::Deserialize;
use serde_json::json;

use crate::domain::{
    llm::LlmToolDefinition,
    llm_tools::{
        get_selected_workout::{build_response_for_date, parse_date, SelectedDate},
        LlmTool, ToolExecutionContext,
    },
};

const GET_SELECTED_WORKOUT_BY_ID_TOOL_NAME: &str = "get_selected_workout_by_id";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetSelectedWorkoutByIdArgs {
    workout_id: String,
}

pub struct GetSelectedWorkoutById;

impl LlmTool for GetSelectedWorkoutById {
    fn name(&self) -> &'static str {
        GET_SELECTED_WORKOUT_BY_ID_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Get detailed workout data for a specific frontend-visible workout id. Use this for the currently selected workout instead of inferring a date from nearby history. Returns completed workouts with statistics and aligned_intervals when a linked planned workout has parseable blocks and a non-empty power stream, heartrate at 1-second resolution when present, plus AI conversation history and related plan/race context for that workout date.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "workout_id": {
                        "type": "string",
                        "description": "Workout id used by the frontend selection context"
                    }
                },
                "required": ["workout_id"]
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "use for the currently selected workout when you need exact workout details or aligned_intervals without guessing the date; aligned_intervals require parseable linked plan blocks and a non-empty power stream",
        )
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let ctx = context.clone();
        Box::pin(async move { get_selected_workout_by_id(&args, &ctx).await })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: GetSelectedWorkoutByIdArgs = serde_json::from_str(arguments_json).ok()?;
        let workout_id = args.workout_id.trim();
        if workout_id.is_empty() {
            return None;
        }
        Some(format!("workout {workout_id}"))
    }

    fn is_available(&self, context: &ToolExecutionContext) -> bool {
        context.data_port.is_some()
    }
}

async fn get_selected_workout_by_id(
    arguments_json: &str,
    context: &ToolExecutionContext,
) -> String {
    let args = match serde_json::from_str::<GetSelectedWorkoutByIdArgs>(arguments_json) {
        Ok(args) if !args.workout_id.trim().is_empty() => args,
        Ok(_) => {
            return json!({ "error": "invalid workout_id: must not be empty" }).to_string();
        }
        Err(err) => {
            return json!({ "error": format!("invalid arguments: {err}") }).to_string();
        }
    };

    let Some(port) = context.data_port.as_ref() else {
        return json!({ "error": "data port not available" }).to_string();
    };

    let data = match port
        .load_selected_workout_data_by_id(&context.user_id, &args.workout_id)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            return json!({
                "error": format!("failed to load selected workout by id: {err}")
            })
            .to_string();
        }
    };

    let response_date = data
        .completed
        .first()
        .map(|workout| {
            workout
                .start_date_local
                .get(..10)
                .unwrap_or_default()
                .to_string()
        })
        .or_else(|| data.planned.first().map(|workout| workout.date.clone()))
        .or_else(|| data.races.first().map(|race| race.date.clone()));

    let Some(response_date) = response_date else {
        return json!({
            "error": format!("no workout data found for workout_id {}", args.workout_id)
        })
        .to_string();
    };

    let Some(parsed_date) = parse_date(&response_date) else {
        return json!({
            "error": format!("invalid response date for workout_id {}: {}", args.workout_id, response_date)
        })
        .to_string();
    };

    build_response_for_date(
        SelectedDate {
            value: response_date,
            parsed: parsed_date,
        },
        data,
        &context.today,
    )
}
