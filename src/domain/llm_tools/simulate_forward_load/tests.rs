use super::SimulateForwardLoad;
use crate::domain::llm_tools::{LlmTool, ToolExecutionContext};
use crate::domain::training_context::PlannedWorkoutContext;
use crate::domain::training_context::{
    AthleteProfileContext, ProjectedDayContext, ProjectedWorkoutContext, TrainingContext,
    UpcomingDayContext,
};

fn sample_context() -> ToolExecutionContext {
    let mut training_context = TrainingContext {
        generated_at_epoch_seconds: 1_700_000_000,
        focus_workout_id: None,
        focus_kind: "summary".to_string(),
        intervals_status: Default::default(),
        profile: AthleteProfileContext {
            availability_configured: true,
            ..Default::default()
        },
        races: Vec::new(),
        planned_rest_days: Vec::new(),
        future_events: Vec::new(),
        history: Default::default(),
        recent_days: Vec::new(),
        recent_workout_recaps: Vec::new(),
        upcoming_days: vec![UpcomingDayContext {
            date: "2026-05-05".to_string(),
            free_day: false,
            planned_workouts: vec![PlannedWorkoutContext {
                event_id: 1,
                start_date_local: "2026-05-05T06:00:00".to_string(),
                name: Some("Morning Endurance".to_string()),
                category: "workout".to_string(),
                interval_blocks: Vec::new(),
                raw_workout_doc: Some("- 60m 65%".to_string()),
                estimated_training_stress_score: Some(65.0),
                estimated_intensity_factor: None,
                estimated_normalized_power_watts: None,
                completed: false,
            }],
            special_days: Vec::new(),
        }],
        projected_days: vec![
            ProjectedDayContext {
                date: "2026-05-06".to_string(),
                workouts: vec![
                    ProjectedWorkoutContext {
                        source_workout_id: "planned-1a".to_string(),
                        start_date_local: "2026-05-06T06:00:00".to_string(),
                        name: Some("Projected Endurance".to_string()),
                        interval_blocks: Vec::new(),
                        raw_workout_doc: Some("- 90m 65%".to_string()),
                        rest_day: false,
                        rest_day_reason: None,
                    },
                    ProjectedWorkoutContext {
                        source_workout_id: "planned-1b".to_string(),
                        start_date_local: "2026-05-06T18:00:00".to_string(),
                        name: Some("Projected Drills".to_string()),
                        interval_blocks: Vec::new(),
                        raw_workout_doc: Some("- 30m easy spin".to_string()),
                        rest_day: false,
                        rest_day_reason: None,
                    },
                ],
            },
            ProjectedDayContext {
                date: "2026-05-07".to_string(),
                workouts: vec![ProjectedWorkoutContext {
                    source_workout_id: "planned-2".to_string(),
                    start_date_local: "2026-05-07T06:00:00".to_string(),
                    name: Some("Projected Tempo".to_string()),
                    interval_blocks: Vec::new(),
                    raw_workout_doc: Some("- 60m 75%".to_string()),
                    rest_day: false,
                    rest_day_reason: None,
                }],
            },
        ],
    };
    training_context.history.ctl = Some(72.0);
    training_context.history.atl = Some(81.0);
    training_context.history.tsb = Some(-9.0);
    training_context.history.ftp_current = Some(300);

    ToolExecutionContext {
        user_id: "user-1".to_string(),
        training_context,
        today: "2026-05-04".to_string(),
        data_port: None,
        planned_workout_update_port: None,
    }
}

#[test]
fn preview_tool_arguments_summarizes_dated_text_range() {
    let tool = SimulateForwardLoad;
    let preview = tool.preview_arguments(
        r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day"}"#,
    );

    assert_eq!(
        preview.as_deref(),
        Some("2 dated days from 2026-05-05 to 2026-05-06")
    );
}

#[test]
fn preview_tool_arguments_uses_singular_day() {
    let tool = SimulateForwardLoad;
    let preview = tool.preview_arguments(r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#);

    assert_eq!(
        preview.as_deref(),
        Some("1 dated day from 2026-05-05 to 2026-05-05")
    );
}

#[test]
fn simulate_forward_load_returns_14_day_sequence() {
    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(tool.execute(
        r#"{"dated_workout_text":"2026-05-05\n- 60m 65%\n2026-05-06\nRest Day: absorb load"}"#,
        &sample_context(),
    ));

    assert!(response.contains("\"baseline\""));
    assert!(response.contains("\"2026-05-05\""));
    assert!(response.contains("\"source\":\"input\""));
    assert!(response.contains("\"source\":\"projected\""));
    assert!(response.contains("\"2026-05-18\""));
}

#[test]
fn simulate_forward_load_aggregates_multiple_projected_workouts() {
    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(tool.execute(
        r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#,
        &sample_context(),
    ));

    // 2026-05-06 has two projected workouts (90m 65% + 30m easy spin).
    assert!(response.contains("\"2026-05-06\""));
    assert!(response.contains("\"source\":\"projected\""));
}

#[test]
fn simulate_forward_load_adds_future_event_tss() {
    let mut ctx = sample_context();
    ctx.training_context.future_events = vec![
        crate::domain::training_context::FuturePlannedEventContext {
            event_id: 1,
            start_date_local: "2026-05-08T08:00:00".to_string(),
            category: "race".to_string(),
            event_type: Some("road".to_string()),
            name: Some("Test Race".to_string()),
            description: None,
            estimated_duration_seconds: Some(7200),
            estimated_training_stress_score: Some(150.0),
            estimated_intensity_factor: None,
            estimated_normalized_power_watts: None,
        },
        crate::domain::training_context::FuturePlannedEventContext {
            event_id: 2,
            start_date_local: "2026-05-08T14:00:00".to_string(),
            category: "race".to_string(),
            event_type: Some("tt".to_string()),
            name: Some("Stage 2".to_string()),
            description: None,
            estimated_duration_seconds: Some(3600),
            estimated_training_stress_score: Some(90.0),
            estimated_intensity_factor: None,
            estimated_normalized_power_watts: None,
        },
    ];

    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(
        tool.execute(r#"{"dated_workout_text":"2026-05-05\n- 60m 65%"}"#, &ctx),
    );

    assert!(response.contains("\"2026-05-08\""));
    assert!(response.contains("\"source\":\"future_event\""));
    assert!(response.contains("\"tss_source\":\"planned\""));
    assert!(!response.contains("race TSS unknown"));
}

#[test]
fn simulate_forward_load_marks_unknown_race_tss_as_none() {
    let mut ctx = sample_context();
    ctx.training_context.future_events =
        vec![crate::domain::training_context::FuturePlannedEventContext {
            event_id: 1,
            start_date_local: "2026-05-08T08:00:00".to_string(),
            category: "race".to_string(),
            event_type: Some("road".to_string()),
            name: Some("Test Race".to_string()),
            description: None,
            estimated_duration_seconds: Some(7200),
            estimated_training_stress_score: None,
            estimated_intensity_factor: None,
            estimated_normalized_power_watts: None,
        }];

    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(tool.execute(r#"{}"#, &ctx));
    let parsed: serde_json::Value = serde_json::from_str(&response).expect("json");

    let race_day = parsed["days"]
        .as_array()
        .unwrap()
        .iter()
        .find(|day| day["date"] == "2026-05-08")
        .expect("race day");
    assert_eq!(race_day["source"], "future_event");
    assert_eq!(race_day["tss_source"], "none");
    assert_eq!(race_day["planned_tss"], 0.0);
    assert!(race_day["tsb"].is_number());

    let notes = parsed["notes"].as_array().expect("notes");
    assert!(notes.iter().any(|note| {
        note.as_str()
            == Some("2026-05-08: race TSS unknown; TSB shown assumes zero race load")
    }));
}

#[test]
fn simulate_forward_load_without_input_uses_context_sources() {
    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(tool.execute(r#"{}"#, &sample_context()));

    // 2026-05-05 should have "upcoming" source (from calendar)
    assert!(response.contains("\"2026-05-05\""));
    assert!(response.contains("\"source\":\"upcoming\""));

    // 2026-05-06 should have "projected" source
    assert!(response.contains("\"2026-05-06\""));
    assert!(response.contains("\"source\":\"projected\""));
}

#[test]
fn simulate_forward_load_input_overrides_upcoming() {
    let ctx = sample_context();
    let tool = SimulateForwardLoad;
    let response = futures::executor::block_on(
        tool.execute(r#"{"dated_workout_text":"2026-05-05\n- 120m 60%"}"#, &ctx),
    );

    // 2026-05-05 should use input, not upcoming
    assert!(response.contains("\"2026-05-05\""));
    assert!(response.contains("\"source\":\"input\""));
}
