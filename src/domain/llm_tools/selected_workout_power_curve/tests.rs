use std::sync::{Arc, Mutex};

use super::SelectedWorkoutPowerCurve;
use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
        CompletedWorkoutPowerCurve, CompletedWorkoutSeries, CompletedWorkoutStream,
    },
    llm_tools::get_selected_workout::SelectedWorkoutData,
    llm_tools::{GetSelectedWorkoutDataPort, LlmTool, ToolExecutionContext},
    training_context::TrainingContext,
};

#[derive(Clone, Default)]
struct TestDataPort {
    workouts: Vec<CompletedWorkout>,
    #[allow(clippy::type_complexity)]
    persisted: Arc<Mutex<Vec<(String, CompletedWorkoutPowerCurve)>>>,
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
        let workouts = self.workouts.clone();
        Box::pin(async move { Ok(workouts) })
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
        Box::pin(async { Ok(Vec::new()) })
    }

    fn load_selected_workout_data_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<SelectedWorkoutData, crate::domain::workout_summary::WorkoutSummaryError>,
    > {
        Box::pin(async {
            Ok(SelectedWorkoutData {
                completed: Vec::new(),
                planned: Vec::new(),
                races: Vec::new(),
                summaries: Vec::new(),
            })
        })
    }

    fn persist_power_curve_5s_if_missing(
        &self,
        _user_id: &str,
        completed_workout_id: &str,
        curve: CompletedWorkoutPowerCurve,
    ) -> crate::domain::completed_workouts::BoxFuture<
        Result<(), crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let persisted = self.persisted.clone();
        let id = completed_workout_id.to_string();
        Box::pin(async move {
            persisted.lock().expect("mutex poisoned").push((id, curve));
            Ok(())
        })
    }
}

fn sample_context(port: TestDataPort) -> ToolExecutionContext {
    ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext {
            generated_at_epoch_seconds: 1,
            focus_workout_id: None,
            focus_kind: "calendar".to_string(),
            intervals_status: Default::default(),
            profile: Default::default(),
            races: Vec::new(),
            planned_rest_days: Vec::new(),
            future_events: Vec::new(),
            history: Default::default(),
            recent_days: Vec::new(),
            recent_workout_recaps: Vec::new(),
            upcoming_days: Vec::new(),
            projected_days: Vec::new(),
        },
        today: "2026-05-05".to_string(),
        data_port: Some(Arc::new(port)),
        planned_workout_update_port: None,
    }
}

fn workout_with_watts(
    id: &str,
    date: &str,
    name: &str,
    watts: Vec<i64>,
    power_curve_5s: Option<CompletedWorkoutPowerCurve>,
) -> CompletedWorkout {
    let mut w = CompletedWorkout::new(
        id.to_string(),
        "user-1".to_string(),
        format!("{date}T12:00:00"),
        None,
        None,
        Some(name.to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics::default(),
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(watts)),
                secondary_series: None,
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
    );
    w.power_curve_5s = power_curve_5s;
    w
}

fn parse_response(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).expect("tool response should be valid JSON")
}

#[tokio::test]
async fn returns_stored_5s_curve_when_present() {
    let stored = CompletedWorkoutPowerCurve {
        resolution_seconds: 5,
        sample_period_seconds: 1,
        source_samples: 10,
        valid_power_samples: 10,
        duration_start_seconds: 5,
        duration_step_seconds: 5,
        max_average_watts: vec![Some(250), Some(230)],
    };
    let workout = workout_with_watts(
        "cw-1",
        "2026-05-05",
        "Threshold",
        vec![200, 250, 300, 275, 225, 180, 240, 260, 255, 215],
        Some(stored),
    );
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["source"], "stored_5s");
    assert_eq!(parsed["max_average_watts"][0], 250);
    assert_eq!(parsed["max_average_watts"][1], 230);
}

#[tokio::test]
async fn computes_5s_ad_hoc_when_cache_missing() {
    let workout = workout_with_watts(
        "cw-1",
        "2026-05-05",
        "Threshold",
        vec![200, 250, 300, 275, 225, 180, 240, 260, 255, 215],
        None,
    );
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port.clone()))
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["source"], "computed_and_persisted_5s");
    assert_eq!(parsed["resolution_seconds"], 5);
    assert!(!parsed["max_average_watts"].as_array().unwrap().is_empty());

    let persisted = port.persisted.lock().expect("mutex poisoned");
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].0, "cw-1");
}

#[tokio::test]
async fn computes_ad_hoc_for_non_5s_resolution() {
    let workout = workout_with_watts(
        "cw-1",
        "2026-05-05",
        "Threshold",
        (0..20).map(|i| 200 + i * 10).collect(),
        None,
    );
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(
            r#"{"date":"2026-05-05","resolution_seconds":10}"#,
            &sample_context(port),
        )
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["source"], "computed_ad_hoc");
    assert_eq!(parsed["resolution_seconds"], 10);
}

#[tokio::test]
async fn returns_error_for_details_unavailable_workout() {
    let mut workout =
        workout_with_watts("cw-1", "2026-05-05", "Threshold", vec![200, 250, 300], None);
    workout.details_unavailable_reason = Some("no fit file".to_string());
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["error"], "power curve unavailable");
    assert!(parsed["reason"].as_str().unwrap().contains("unavailable"));
}

#[tokio::test]
async fn returns_error_for_missing_watts_stream() {
    let workout = CompletedWorkout::new(
        "cw-1".to_string(),
        "user-1".to_string(),
        "2026-05-05T12:00:00".to_string(),
        None,
        None,
        Some("Threshold".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics::default(),
        CompletedWorkoutDetails {
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
        None,
    );
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["error"], "power curve unavailable");
    assert!(parsed["reason"].as_str().unwrap().contains("watts"));
}

#[tokio::test]
async fn rejects_invalid_resolution() {
    let workout = workout_with_watts(
        "cw-1",
        "2026-05-05",
        "Threshold",
        vec![200, 250, 300, 275, 225],
        None,
    );
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(
            r#"{"date":"2026-05-05","resolution_seconds":7}"#,
            &sample_context(port),
        )
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["error"], "invalid resolution");
}

#[tokio::test]
async fn returns_error_for_no_workouts_on_date() {
    let port = TestDataPort {
        workouts: Vec::new(),
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert_eq!(
        parsed["error"],
        "no completed workouts found for date 2026-05-05"
    );
}

#[tokio::test]
async fn requires_workout_id_for_multiple_workouts() {
    let w1 = workout_with_watts("cw-1", "2026-05-05", "Workout A", vec![200, 250, 300], None);
    let w2 = workout_with_watts("cw-2", "2026-05-05", "Workout B", vec![150, 200, 250], None);
    let port = TestDataPort {
        workouts: vec![w1, w2],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"2026-05-05"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert!(parsed["error"]
        .as_str()
        .unwrap()
        .contains("multiple completed workouts"));
    assert!(parsed["error"].as_str().unwrap().contains("cw-1"));
    assert!(parsed["error"].as_str().unwrap().contains("cw-2"));
}

#[tokio::test]
async fn selects_specific_workout_by_id() {
    let w1 = workout_with_watts("cw-1", "2026-05-05", "Workout A", vec![200, 250, 300], None);
    let w2 = workout_with_watts("cw-2", "2026-05-05", "Workout B", vec![150, 200, 250], None);
    let port = TestDataPort {
        workouts: vec![w1, w2],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(
            r#"{"date":"2026-05-05","workout_id":"cw-2"}"#,
            &sample_context(port),
        )
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["workout_id"], "cw-2");
    assert_eq!(parsed["workout_name"], "Workout B");
}

#[tokio::test]
async fn rejects_unknown_fields() {
    let workout = workout_with_watts("cw-1", "2026-05-05", "Threshold", vec![200, 250, 300], None);
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(
            r#"{"date":"2026-05-05","unknown_field":123}"#,
            &sample_context(port),
        )
        .await;
    let parsed = parse_response(&result);
    assert!(parsed["error"]
        .as_str()
        .unwrap()
        .contains("invalid arguments"));
}

#[tokio::test]
async fn rejects_invalid_date() {
    let port = TestDataPort {
        workouts: Vec::new(),
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(r#"{"date":"not-a-date"}"#, &sample_context(port))
        .await;
    let parsed = parse_response(&result);
    assert!(parsed["error"].as_str().unwrap().contains("invalid date"));
}

#[tokio::test]
async fn returns_error_for_resolution_larger_than_data() {
    let workout = workout_with_watts("cw-1", "2026-05-05", "Threshold", vec![200, 250, 300], None);
    let port = TestDataPort {
        workouts: vec![workout],
        ..Default::default()
    };

    let tool = SelectedWorkoutPowerCurve;
    let result = tool
        .execute(
            r#"{"date":"2026-05-05","resolution_seconds":10}"#,
            &sample_context(port),
        )
        .await;
    let parsed = parse_response(&result);
    assert_eq!(parsed["error"], "power curve unavailable");
    assert!(parsed["reason"]
        .as_str()
        .unwrap()
        .contains("not enough data"));
}

#[test]
fn tool_definition_schema_is_parseable() {
    let tool = SelectedWorkoutPowerCurve;
    let def = tool.definition();
    let schema: serde_json::Value =
        serde_json::from_str(&def.input_schema_json).expect("schema should be valid JSON");
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn is_available_when_data_port_present() {
    let port = TestDataPort {
        workouts: Vec::new(),
        ..Default::default()
    };
    let context = sample_context(port);

    let tool = SelectedWorkoutPowerCurve;
    assert!(tool.is_available(&context));
}

#[test]
fn not_available_when_data_port_missing() {
    let context = ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context: TrainingContext {
            generated_at_epoch_seconds: 1,
            focus_workout_id: None,
            focus_kind: "calendar".to_string(),
            intervals_status: Default::default(),
            profile: Default::default(),
            races: Vec::new(),
            planned_rest_days: Vec::new(),
            future_events: Vec::new(),
            history: Default::default(),
            recent_days: Vec::new(),
            recent_workout_recaps: Vec::new(),
            upcoming_days: Vec::new(),
            projected_days: Vec::new(),
        },
        today: "2026-05-05".to_string(),
        data_port: None,
        planned_workout_update_port: None,
    };

    let tool = SelectedWorkoutPowerCurve;
    assert!(!tool.is_available(&context));
}

#[test]
fn preview_arguments_displays_date_and_resolution() {
    let tool = SelectedWorkoutPowerCurve;
    let preview = tool
        .preview_arguments(r#"{"date":"2026-05-05","resolution_seconds":10}"#)
        .expect("should parse");
    assert!(preview.contains("2026-05-05"));
    assert!(preview.contains("10s"));
}

#[test]
fn preview_arguments_includes_workout_id() {
    let tool = SelectedWorkoutPowerCurve;
    let preview = tool
        .preview_arguments(r#"{"date":"2026-05-05","workout_id":"cw-2"}"#)
        .expect("should parse");
    assert!(preview.contains("2026-05-05"));
    assert!(preview.contains("cw-2"));
}
