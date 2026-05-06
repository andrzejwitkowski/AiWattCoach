use std::sync::Arc;

use super::GetSelectedWorkout;
use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream,
    },
    llm::LlmChatMessage,
    llm_tools::{GetSelectedWorkoutDataPort, LlmTool, ToolExecutionContext},
    planned_workouts::{PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine},
    races::{Race, RaceDiscipline, RacePriority},
    training_context::TrainingContext,
    workout_summary::{ConversationMessage, MessageRole, WorkoutSummary},
};

#[test]
fn preview_tool_arguments_shows_date() {
    let tool = GetSelectedWorkout;
    let preview = tool.preview_arguments(r#"{"date":"2026-05-05"}"#);
    assert_eq!(preview.as_deref(), Some("date 2026-05-05"));
}

#[test]
fn preview_tool_arguments_rejects_invalid_date() {
    let tool = GetSelectedWorkout;
    let preview = tool.preview_arguments(r#"{"date":"05-05-2026"}"#);
    assert_eq!(preview, None);
}

#[test]
fn get_selected_workout_returns_completed_data_and_hides_race() {
    let tool = GetSelectedWorkout;
    let context = sample_context(TestDataPort {
        completed: vec![sample_completed_workout(Some("planned-1"))],
        planned: vec![sample_planned_workout("planned-1", "2026-05-05")],
        races: vec![sample_race("2026-05-05")],
        summaries: vec![sample_summary("completed-1")],
    });

    let response = futures::executor::block_on(tool.execute(r#"{"date":"2026-05-05"}"#, &context));
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid json");
    let workout = &json["workouts"][0];
    let stream = &workout["streams"][0];
    let conversation = &workout["ai_conversation"][0];

    assert_eq!(workout["kind"], "completed");
    assert_eq!(workout["workout_id"], "completed-1");
    assert_eq!(stream["stream_type"], "watts");
    assert_eq!(stream["data"], serde_json::json!([250, 260, 270]));
    assert_eq!(stream["secondary_data"], serde_json::json!([251, 261, 271]));
    assert_eq!(conversation["role"], "coach");
    assert_eq!(workout["ai_summary"], "Strong threshold execution");
    assert!(workout["metrics"]["variability_index"].is_null());
    assert!(workout["metrics"]["total_work_joules"].is_null());
    assert!(workout["metrics"]["strain_score"].is_null());
    assert_eq!(json["races"], serde_json::json!([]));
    assert_ne!(workout["kind"], "planned");
}

#[test]
fn get_selected_workout_downsamples_large_streams() {
    let tool = GetSelectedWorkout;
    let mut workout = sample_completed_workout(None);
    workout.details.streams[0].primary_series = Some(CompletedWorkoutSeries::Integers(
        (0_i64..1000_i64).collect(),
    ));
    let context = sample_context(TestDataPort {
        completed: vec![workout],
        planned: Vec::new(),
        races: Vec::new(),
        summaries: Vec::new(),
    });

    let response = futures::executor::block_on(tool.execute(r#"{"date":"2026-05-05"}"#, &context));
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid json");
    let stream_data = json["workouts"][0]["streams"][0]["data"]
        .as_array()
        .expect("stream data should be an array");

    assert!(stream_data.len() <= 256);
}

#[test]
fn get_selected_workout_maps_non_finite_float_stream_values_to_null() {
    let tool = GetSelectedWorkout;
    let mut workout = sample_completed_workout(None);
    workout.details.streams[0].primary_series = Some(CompletedWorkoutSeries::Floats(vec![
        123.4,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ]));
    let context = sample_context(TestDataPort {
        completed: vec![workout],
        planned: Vec::new(),
        races: Vec::new(),
        summaries: Vec::new(),
    });

    let response = futures::executor::block_on(tool.execute(r#"{"date":"2026-05-05"}"#, &context));
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("response should be valid json");

    assert_eq!(
        json["workouts"][0]["streams"][0]["data"],
        serde_json::json!([123.4, null, null, null])
    );
}

#[test]
fn get_selected_workout_marks_past_uncompleted_plan() {
    let tool = GetSelectedWorkout;
    let context = sample_context(TestDataPort {
        completed: Vec::new(),
        planned: vec![sample_planned_workout("planned-1", "2026-05-04")],
        races: Vec::new(),
        summaries: Vec::new(),
    });

    let response = futures::executor::block_on(tool.execute(r#"{"date":"2026-05-04"}"#, &context));

    assert!(response.contains(r#""kind":"planned""#));
    assert!(response.contains(r#""status":"not_completed""#));
    assert!(response.contains(r#""raw_workout_doc":"- 60m 65%""#));
}

#[test]
fn get_selected_workout_returns_basic_race_when_no_completed_workout_exists() {
    let tool = GetSelectedWorkout;
    let context = sample_context(TestDataPort {
        completed: Vec::new(),
        planned: Vec::new(),
        races: vec![sample_race("2026-05-05")],
        summaries: Vec::new(),
    });

    let response = futures::executor::block_on(tool.execute(r#"{"date":"2026-05-05"}"#, &context));

    assert!(response.contains(r#""race_id":"race-1""#));
    assert!(response.contains(r#""discipline":"road""#));
    assert!(response.contains(r#""priority":"A""#));
}

#[test]
fn get_selected_workout_returns_error_for_invalid_date() {
    let tool = GetSelectedWorkout;
    let context = sample_context(TestDataPort::default());

    let response = futures::executor::block_on(tool.execute(r#"{"date":"05-05-2026"}"#, &context));

    assert!(response.contains("invalid date: expected YYYY-MM-DD"));
}

#[derive(Clone, Default)]
struct TestDataPort {
    completed: Vec<CompletedWorkout>,
    planned: Vec<PlannedWorkout>,
    races: Vec<Race>,
    summaries: Vec<WorkoutSummary>,
}

impl GetSelectedWorkoutDataPort for TestDataPort {
    fn list_completed_by_date_range(
        &self,
        _user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let completed = self.completed.clone();
        Box::pin(async move {
            Ok(completed
                .into_iter()
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .collect())
        })
    }

    fn list_planned_by_date_range(
        &self,
        _user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::planned_workouts::BoxFuture<
        Result<Vec<PlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let planned = self.planned.clone();
        Box::pin(async move {
            Ok(planned
                .into_iter()
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .collect())
        })
    }

    fn list_races_by_date_range(
        &self,
        _user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, crate::domain::races::RaceError>> {
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let races = self.races.clone();
        Box::pin(async move {
            Ok(races
                .into_iter()
                .filter(|race| race.date >= oldest && race.date <= newest)
                .collect())
        })
    }

    fn find_summaries_by_workout_ids(
        &self,
        _user_id: &str,
        workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Vec<WorkoutSummary>, crate::domain::workout_summary::WorkoutSummaryError>,
    > {
        let summaries = self.summaries.clone();
        Box::pin(async move {
            Ok(summaries
                .into_iter()
                .filter(|summary| workout_ids.contains(&summary.workout_id))
                .collect())
        })
    }
}

fn sample_context(data_port: TestDataPort) -> ToolExecutionContext {
    ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext {
            generated_at_epoch_seconds: 1,
            focus_workout_id: None,
            focus_kind: "calendar".to_string(),
            intervals_status: Default::default(),
            profile: Default::default(),
            races: Vec::new(),
            future_events: Vec::new(),
            history: Default::default(),
            recent_days: Vec::new(),
            upcoming_days: Vec::new(),
            projected_days: Vec::new(),
        },
        today: "2026-05-05".to_string(),
        data_port: Some(Arc::new(data_port)),
    }
}

fn sample_completed_workout(planned_workout_id: Option<&str>) -> CompletedWorkout {
    CompletedWorkout::new(
        "completed-1".to_string(),
        "user-1".to_string(),
        "2026-05-05T08:00:00".to_string(),
        None,
        planned_workout_id.map(str::to_string),
        Some("Threshold Intervals".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        true,
        Some(3600),
        Some(30_000.0),
        CompletedWorkoutMetrics {
            training_stress_score: Some(95),
            normalized_power_watts: Some(280),
            intensity_factor: Some(0.85),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(250),
            ftp_watts: Some(330),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![250, 260, 270])),
                secondary_series: Some(CompletedWorkoutSeries::Integers(vec![251, 261, 271])),
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

fn sample_planned_workout(id: &str, date: &str) -> PlannedWorkout {
    PlannedWorkout::new(
        id.to_string(),
        "user-1".to_string(),
        date.to_string(),
        PlannedWorkoutContent {
            lines: vec![PlannedWorkoutLine::Step(
                crate::domain::planned_workouts::PlannedWorkoutStep {
                    duration_seconds: 3600,
                    kind: crate::domain::planned_workouts::PlannedWorkoutStepKind::Steady,
                    target: crate::domain::planned_workouts::PlannedWorkoutTarget::PercentFtp {
                        min: 65.0,
                        max: 65.0,
                    },
                },
            )],
        },
    )
    .with_event_metadata(Some("Planned Endurance".to_string()), None, None)
}

fn sample_race(date: &str) -> Race {
    Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: date.to_string(),
        name: "A Race".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::A,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

fn sample_summary(workout_id: &str) -> WorkoutSummary {
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
            created_at_epoch_seconds: 1,
        }],
        provider_transcript: vec![LlmChatMessage::assistant("Great threshold work")],
        saved_at_epoch_seconds: Some(1),
        workout_recap_text: Some("Strong threshold execution".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("gemini".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}
