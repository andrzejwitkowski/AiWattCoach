use std::{future::Future, pin::Pin};

use serde::Deserialize;
use serde_json::json;

use crate::domain::{
    external_sync::ExternalProvider,
    llm::LlmToolDefinition,
    llm_tools::{LlmTool, ToolExecutionContext},
    planned_workouts::{
        UpdatePlannedWorkoutCommand, UpdatePlannedWorkoutError, UpdatePlannedWorkoutOutcome,
    },
};

pub trait UpdatePlannedWorkoutDataPort: Send + Sync {
    fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError>>
                + Send,
        >,
    >;
}

pub(crate) const UPDATE_PLANNED_WORKOUT_TOOL_NAME: &str = "update_planned_workout";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdatePlannedWorkoutArgs {
    date: String,
    planned_workout_id: String,
    workout_doc: String,
}

pub struct UpdatePlannedWorkout;

impl LlmTool for UpdatePlannedWorkout {
    fn name(&self) -> &'static str {
        UPDATE_PLANNED_WORKOUT_TOOL_NAME
    }

    fn definition(&self) -> LlmToolDefinition {
        LlmToolDefinition {
            name: self.name().to_string(),
            description: "Replace an already planned workout for a specific date and plannedWorkoutId using Intervals.icu workout-builder text. Use this only in the calendar AI coach after the user explicitly confirmed they want to overwrite the existing planned workout. If the workout was already synced to Intervals.icu and/or Wahoo, this tool updates the existing remote entries instead of creating new ones.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "date": {
                        "type": "string",
                        "description": "Date in YYYY-MM-DD format"
                    },
                    "plannedWorkoutId": {
                        "type": "string",
                        "description": "Exact planned workout id to replace"
                    },
                    "workoutDoc": {
                        "type": "string",
                        "description": "Replacement workout in Intervals.icu workout-builder text format"
                    }
                },
                "required": ["date", "plannedWorkoutId", "workoutDoc"]
            })
            .to_string(),
        }
    }

    fn prompt_guidance(&self) -> Option<&'static str> {
        Some(
            "use only after the user explicitly confirmed they want to replace an already planned workout; always send the exact `date`, exact `plannedWorkoutId`, and the full replacement Intervals-style `workoutDoc`",
        )
    }

    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>> {
        let args = arguments_json.to_string();
        let context = context.clone();
        Box::pin(async move { update_planned_workout(&args, &context).await })
    }

    fn preview_arguments(&self, arguments_json: &str) -> Option<String> {
        let args: UpdatePlannedWorkoutArgs = serde_json::from_str(arguments_json).ok()?;
        if chrono::NaiveDate::parse_from_str(&args.date, "%Y-%m-%d").is_err() {
            return None;
        }
        Some(format!(
            "replace {} on {}",
            args.planned_workout_id, args.date
        ))
    }

    fn is_available(&self, context: &ToolExecutionContext) -> bool {
        context.planned_workout_update_port.is_some()
    }
}

async fn update_planned_workout(arguments_json: &str, context: &ToolExecutionContext) -> String {
    let args = match serde_json::from_str::<UpdatePlannedWorkoutArgs>(arguments_json) {
        Ok(args) => args,
        Err(error) => return json!({ "error": format!("invalid arguments: {error}") }).to_string(),
    };
    let Some(port) = context.planned_workout_update_port.as_ref() else {
        return json!({ "error": "planned workout update port not available" }).to_string();
    };

    let command = UpdatePlannedWorkoutCommand {
        user_id: context.user_id.clone(),
        planned_workout_id: args.planned_workout_id,
        date: args.date,
        workout_doc: args.workout_doc,
    };

    match port.update_planned_workout(command).await {
        Ok(outcome) => json!({
            "plannedWorkoutId": outcome.planned_workout.planned_workout_id,
            "date": outcome.planned_workout.date,
            "syncedProviders": outcome
                .synced_providers
                .into_iter()
                .map(|provider| match provider {
                    ExternalProvider::Intervals => "intervals",
                    ExternalProvider::Wahoo => "wahoo",
                    ExternalProvider::Strava => "strava",
                    ExternalProvider::Other => "other",
                })
                .collect::<Vec<_>>(),
            "failedProviders": outcome
                .failed_providers
                .into_iter()
                .map(|failure| json!({
                    "provider": match failure.provider {
                        ExternalProvider::Intervals => "intervals",
                        ExternalProvider::Wahoo => "wahoo",
                        ExternalProvider::Strava => "strava",
                        ExternalProvider::Other => "other",
                    },
                    "error": failure.error,
                }))
                .collect::<Vec<_>>(),
            "status": "updated"
        })
        .to_string(),
        Err(error) => json!({ "error": error.to_string() }).to_string(),
    }
}
