use std::sync::{Arc, Mutex};

use super::{
    update_planned_workout::{UpdatePlannedWorkout, UpdatePlannedWorkoutDataPort},
    LlmTool, ToolExecutionContext,
};
use crate::domain::{
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, UpdatePlannedWorkoutCommand,
        UpdatePlannedWorkoutError, UpdatePlannedWorkoutOutcome,
    },
    training_context::TrainingContext,
};

#[derive(Clone, Default)]
struct RecordingPort {
    commands: Arc<Mutex<Vec<UpdatePlannedWorkoutCommand>>>,
}

impl UpdatePlannedWorkoutDataPort for RecordingPort {
    fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError>,
                > + Send,
        >,
    > {
        let commands = self.commands.clone();
        Box::pin(async move {
            commands
                .lock()
                .expect("mutex poisoned")
                .push(command.clone());
            Ok(UpdatePlannedWorkoutOutcome {
                planned_workout: PlannedWorkout::new(
                    command.planned_workout_id,
                    command.user_id,
                    command.date,
                    PlannedWorkoutContent { lines: Vec::new() },
                ),
                synced_providers: Vec::new(),
            })
        })
    }
}

fn sample_context() -> ToolExecutionContext {
    ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext::default(),
        today: "2026-05-05".to_string(),
        data_port: None,
        planned_workout_update_port: Some(Arc::new(RecordingPort::default())),
    }
}

#[test]
fn update_planned_workout_tool_description_mentions_explicit_confirmation() {
    let tool = UpdatePlannedWorkout;

    assert!(tool
        .definition()
        .description
        .contains("explicitly confirmed"));
    assert!(tool.definition().description.contains("calendar AI coach"));
}

#[test]
fn update_planned_workout_tool_is_available_only_with_update_port() {
    let tool = UpdatePlannedWorkout;
    let available = sample_context();
    let unavailable = ToolExecutionContext {
        planned_workout_update_port: None,
        ..sample_context()
    };

    assert!(tool.is_available(&available));
    assert!(!tool.is_available(&unavailable));
}

#[test]
fn update_planned_workout_preview_requires_valid_date() {
    let tool = UpdatePlannedWorkout;

    assert_eq!(
        tool.preview_arguments(
            r#"{"date":"2026-05-05","plannedWorkoutId":"pw-1","workoutDoc":"Warmup"}"#,
        )
        .as_deref(),
        Some("replace pw-1 on 2026-05-05"),
    );
    assert!(tool
        .preview_arguments(
            r#"{"date":"05-05-2026","plannedWorkoutId":"pw-1","workoutDoc":"Warmup"}"#,
        )
        .is_none());
}
