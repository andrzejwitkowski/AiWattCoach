use std::sync::Arc;

use super::{
    get_selected_workout::SelectedWorkoutData, GetSelectedWorkoutById, GetSelectedWorkoutDataPort,
    LlmTool, ToolExecutionContext,
};
use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics},
    llm::LlmChatMessage,
    training_context::TrainingContext,
    workout_summary::{ConversationMessage, MessageRole, WorkoutSummary},
};

#[derive(Clone, Default)]
struct TestDataPort {
    completed: Vec<CompletedWorkout>,
    summaries: Vec<WorkoutSummary>,
}

impl GetSelectedWorkoutDataPort for TestDataPort {
    fn list_completed_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_planned_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<
            Vec<crate::domain::planned_workouts::PlannedWorkout>,
            crate::domain::planned_workouts::PlannedWorkoutError,
        >,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_races_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> crate::domain::races::BoxFuture<
        Result<Vec<crate::domain::races::Race>, crate::domain::races::RaceError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_summaries_by_workout_ids(
        &self,
        _user_id: &str,
        _workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<
            Vec<crate::domain::workout_summary::WorkoutSummary>,
            crate::domain::workout_summary::WorkoutSummaryError,
        >,
    > {
        let summaries = self.summaries.clone();
        Box::pin(async move { Ok(summaries) })
    }

    fn load_selected_workout_data_by_id(
        &self,
        _user_id: &str,
        workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<SelectedWorkoutData, crate::domain::workout_summary::WorkoutSummaryError>,
    > {
        let completed = self.completed.clone();
        let summaries = self.summaries.clone();
        let workout_id = workout_id.to_string();

        Box::pin(async move {
            Ok(SelectedWorkoutData {
                completed: completed
                    .into_iter()
                    .filter(|workout| {
                        workout.source_activity_id.as_deref() == Some(workout_id.as_str())
                    })
                    .collect(),
                planned: Vec::new(),
                races: Vec::new(),
                summaries,
            })
        })
    }
}

#[test]
fn preview_tool_arguments_shows_workout_id() {
    let tool = GetSelectedWorkoutById;
    let preview = tool.preview_arguments(r#"{"workout_id":"activity-123"}"#);

    assert_eq!(preview.as_deref(), Some("workout activity-123"));
}

#[test]
fn get_selected_workout_by_id_returns_completed_data_for_frontend_workout_id() {
    let tool = GetSelectedWorkoutById;
    let workout = CompletedWorkout {
        completed_workout_id: "completed-1".to_string(),
        user_id: "user-1".to_string(),
        start_date_local: "2026-05-05T08:00:00".to_string(),
        source_activity_id: Some("activity-123".to_string()),
        planned_workout_id: None,
        name: Some("Threshold Intervals".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        external_id: None,
        trainer: true,
        duration_seconds: Some(3600),
        distance_meters: Some(30_000.0),
        metrics: CompletedWorkoutMetrics::default(),
        details: CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
        power_curve_5s: None,
    };
    let context = ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext::default(),
        today: "2026-05-05".to_string(),
        data_port: Some(Arc::new(TestDataPort {
            completed: vec![workout],
            summaries: vec![sample_summary()],
        })),
        planned_workout_update_port: None,
    };

    let response =
        futures::executor::block_on(tool.execute(r#"{"workout_id":"activity-123"}"#, &context));
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid json");

    assert_eq!(json["date"], "2026-05-05");
    assert_eq!(json["workouts"][0]["kind"], "completed");
    assert_eq!(json["workouts"][0]["workout_id"], "completed-1");
}

#[test]
fn get_selected_workout_by_id_returns_summary_when_saved_under_alias_workout_id() {
    let tool = GetSelectedWorkoutById;
    let workout = CompletedWorkout {
        completed_workout_id: "completed-1".to_string(),
        user_id: "user-1".to_string(),
        start_date_local: "2026-05-05T08:00:00".to_string(),
        source_activity_id: Some("activity-123".to_string()),
        planned_workout_id: None,
        name: Some("Threshold Intervals".to_string()),
        description: None,
        activity_type: Some("Ride".to_string()),
        external_id: Some("external-456".to_string()),
        trainer: true,
        duration_seconds: Some(3600),
        distance_meters: Some(30_000.0),
        metrics: CompletedWorkoutMetrics::default(),
        details: CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
        power_curve_5s: None,
    };
    let context = ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext::default(),
        today: "2026-05-05".to_string(),
        data_port: Some(Arc::new(TestDataPort {
            completed: vec![workout],
            summaries: vec![sample_summary_with_workout_id("external-456")],
        })),
        planned_workout_update_port: None,
    };

    let response =
        futures::executor::block_on(tool.execute(r#"{"workout_id":"activity-123"}"#, &context));
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid json");

    assert_eq!(
        json["workouts"][0]["ai_summary"],
        "Strong threshold execution"
    );
    assert_eq!(
        json["workouts"][0]["ai_conversation"][0]["content"],
        "Great threshold work"
    );
}

#[test]
fn get_selected_workout_by_id_returns_error_for_missing_workout() {
    let tool = GetSelectedWorkoutById;
    let context = ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext::default(),
        today: "2026-05-05".to_string(),
        data_port: Some(Arc::new(TestDataPort::default())),
        planned_workout_update_port: None,
    };

    let response =
        futures::executor::block_on(tool.execute(r#"{"workout_id":"missing"}"#, &context));

    assert!(response.contains("no workout data found for workout_id missing"));
}

fn sample_summary() -> WorkoutSummary {
    sample_summary_with_workout_id("completed-1")
}

fn sample_summary_with_workout_id(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(7),
        messages: vec![ConversationMessage {
            id: "message-1".to_string(),
            role: MessageRole::Coach,
            content: "Great threshold work".to_string(),
            tool_call: None,
            questions: Vec::new(),
            created_at_epoch_seconds: 1,
            image_url: None,
        }],
        provider_transcript: vec![LlmChatMessage::assistant(
            crate::domain::workout_summary::coach_reply_json("Great threshold work"),
        )],
        saved_at_epoch_seconds: Some(1),
        workout_recap_text: Some("Strong threshold execution".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("gemini".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}
