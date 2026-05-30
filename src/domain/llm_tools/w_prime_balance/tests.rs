use super::*;
use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream,
    },
    training_context::TrainingContext,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn empty_details() -> CompletedWorkoutDetails {
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
    }
}

fn details_with_stream(stream: CompletedWorkoutStream) -> CompletedWorkoutDetails {
    CompletedWorkoutDetails {
        streams: vec![stream],
        ..empty_details()
    }
}

fn watts_stream(values: Vec<i64>) -> CompletedWorkoutStream {
    CompletedWorkoutStream {
        stream_type: "watts".to_string(),
        name: None,
        primary_series: Some(CompletedWorkoutSeries::Integers(values)),
        secondary_series: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }
}

fn make_workout(watts: Vec<i64>) -> CompletedWorkout {
    CompletedWorkout::new(
        "test-workout".to_string(),
        "user-1".to_string(),
        "2026-01-01T12:00:00".to_string(),
        None,
        None,
        Some("Test Ride".to_string()),
        None,
        None,
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics::default(),
        details_with_stream(watts_stream(watts)),
        None,
    )
}

fn make_workout_no_streams() -> CompletedWorkout {
    CompletedWorkout::new(
        "test-workout".to_string(),
        "user-1".to_string(),
        "2026-01-01T12:00:00".to_string(),
        None,
        None,
        Some("No Streams".to_string()),
        None,
        None,
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics::default(),
        empty_details(),
        None,
    )
}

fn test_context(ftp: Option<i32>, weight_kg: Option<f64>) -> ToolExecutionContext {
    let mut tc = TrainingContext::default();
    tc.history.ftp_current = ftp;
    tc.profile.weight_kg = weight_kg;
    ToolExecutionContext {
        user_id: "test-user".to_string(),
        training_context: tc,
        today: "2026-05-05".to_string(),
        data_port: None,
        planned_workout_update_port: None,
    }
}

fn args(date: &str) -> WPrimeBalanceArgs {
    WPrimeBalanceArgs {
        date: date.to_string(),
        workout_id: None,
        cp_watts: None,
        w_prime_joules: None,
    }
}

// ---------------------------------------------------------------------------
// extract_power_samples
// ---------------------------------------------------------------------------

#[test]
fn extract_power_samples_integers() {
    let workout = make_workout(vec![100, 200, 300]);
    let samples = extract_power_samples(&workout).unwrap();
    assert_eq!(samples, vec![Some(100), Some(200), Some(300)]);
}

#[test]
fn extract_power_samples_negative_to_none() {
    let workout = make_workout(vec![-1, 0, 100]);
    let samples = extract_power_samples(&workout).unwrap();
    assert_eq!(samples, vec![None, Some(0), Some(100)]);
}

#[test]
fn extract_power_samples_large_integer_to_none() {
    let workout = make_workout(vec![i32::MAX as i64 + 1, 250]);
    let samples = extract_power_samples(&workout).unwrap();
    assert_eq!(samples, vec![None, Some(250)]);
}

#[test]
fn extract_power_samples_no_watts_stream() {
    let workout = make_workout_no_streams();
    let result = extract_power_samples(&workout);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no watts"));
}

// ---------------------------------------------------------------------------
// estimate_cp
// ---------------------------------------------------------------------------

#[test]
fn estimate_cp_user_provided() {
    let ctx = test_context(Some(300), None);
    let mut a = args("2026-01-01");
    a.cp_watts = Some(280);
    let (cp, source) = estimate_cp(&a, &ctx);
    assert_eq!(cp, 280);
    assert_eq!(source, "user_provided");
}

#[test]
fn estimate_cp_from_ftp() {
    let ctx = test_context(Some(300), None);
    let a = args("2026-01-01");
    let (cp, source) = estimate_cp(&a, &ctx);
    assert_eq!(cp, 270); // floor(300 * 0.90)
    assert_eq!(source, "estimated_from_ftp");
}

#[test]
fn estimate_cp_default_when_no_ftp() {
    let ctx = test_context(None, None);
    let a = args("2026-01-01");
    let (cp, source) = estimate_cp(&a, &ctx);
    assert_eq!(cp, DEFAULT_CP_WATTS);
    assert_eq!(source, "default");
}

#[test]
fn estimate_cp_default_when_ftp_zero() {
    let ctx = test_context(Some(0), None);
    let a = args("2026-01-01");
    let (cp, source) = estimate_cp(&a, &ctx);
    assert_eq!(cp, DEFAULT_CP_WATTS);
    assert_eq!(source, "default");
}

// ---------------------------------------------------------------------------
// estimate_w_prime
// ---------------------------------------------------------------------------

#[test]
fn estimate_w_prime_user_provided() {
    let ctx = test_context(None, None);
    let mut a = args("2026-01-01");
    a.w_prime_joules = Some(22000);
    let (wp, source) = estimate_w_prime(&a, &ctx);
    assert_eq!(wp, 22000);
    assert_eq!(source, "user_provided");
}

#[test]
fn estimate_w_prime_from_weight() {
    let ctx = test_context(None, Some(70.0));
    let a = args("2026-01-01");
    let (wp, source) = estimate_w_prime(&a, &ctx);
    assert_eq!(wp, 19600); // floor(70.0 * 280.0)
    assert_eq!(source, "estimated_from_weight");
}

#[test]
fn estimate_w_prime_default_when_no_weight() {
    let ctx = test_context(None, None);
    let a = args("2026-01-01");
    let (wp, source) = estimate_w_prime(&a, &ctx);
    assert_eq!(wp, DEFAULT_W_PRIME_JOULES);
    assert_eq!(source, "default");
}

#[test]
fn estimate_w_prime_default_when_weight_zero() {
    let ctx = test_context(None, Some(0.0));
    let a = args("2026-01-01");
    let (wp, source) = estimate_w_prime(&a, &ctx);
    assert_eq!(wp, DEFAULT_W_PRIME_JOULES);
    assert_eq!(source, "default");
}

// ---------------------------------------------------------------------------
// compute_w_prime_balance — algorithm correctness
// ---------------------------------------------------------------------------

#[test]
fn balance_stays_full_when_power_below_cp() {
    // 5 seconds at 200W, CP=250 -> no expenditure, already fully charged
    let samples: Vec<Option<i32>> = vec![Some(200); 5];
    let (series, start, end, min, deficit, a90, s50, s10, b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!((start - 20_000.0).abs() < 0.1);
    assert!((end - 20_000.0).abs() < 0.1);
    assert!((min - 20_000.0).abs() < 0.1);
    assert!((deficit - 0.0).abs() < 0.1);
    assert_eq!(a90, 5);
    assert_eq!(s50, 0);
    assert_eq!(s10, 0);
    assert_eq!(b10, 0);
    assert!(!depleted);
    assert_eq!(series.len(), 5);
}

#[test]
fn balance_depletes_linearly_above_cp() {
    // 10 seconds at 300W, CP=250 -> expenditure = 50 J/s
    let samples: Vec<Option<i32>> = vec![Some(300); 10];
    let (series, _start, end, min, deficit, a90, s50, s10, b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    let expected_end = 20_000.0 - 10.0 * 50.0;
    assert!((end - expected_end).abs() < 0.1);
    assert!((min - expected_end).abs() < 0.1);
    assert!((deficit - 500.0).abs() < 0.1);
    assert!(!depleted);
    assert_eq!(a90, 10);
    assert_eq!(s50, 0);
    assert_eq!(s10, 0);
    assert_eq!(b10, 0);
    assert_eq!(series.len(), 10);
}

#[test]
fn balance_never_goes_below_zero() {
    // 500 seconds at 300W, CP=250, W'=20000 -> would be -5000 without clamp
    let samples: Vec<Option<i32>> = vec![Some(300); 500];
    let (series, _start, end, min, deficit, _a90, _s50, _s10, _b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!((end - 0.0).abs() < 0.1);
    assert!((min - 0.0).abs() < 0.1);
    assert!((deficit - 20_000.0).abs() < 0.1);
    assert!(depleted);
    assert!(series.iter().all(|&v| v >= -0.01));
}

#[test]
fn balance_recovers_exponentially_below_cp() {
    // 100s at 300W (deplete), then 100s at 200W (recover), CP=250, W'=20000
    let mut samples = vec![Some(300); 100];
    samples.extend(vec![Some(200); 100]);

    let (series, _start, end, min, deficit, _a90, _s50, _s10, _b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    let expected_after_depletion = 15_000.0;
    let expected_end = 20_000.0 - 5_000.0 * (-0.25f64).exp();

    assert!(!depleted);
    assert!((end - expected_end).abs() < 1.0);
    assert!((min - expected_after_depletion).abs() < 1.0);
    assert!(end > min + 1000.0);
    assert!((deficit - 5_000.0).abs() < 1.0);
    assert_eq!(series.len(), 200);
}

#[test]
fn balance_does_not_recover_beyond_w_prime() {
    // 50s at 300W, then 1000s at 0W (coasting), CP=250, W'=20000
    let mut samples = vec![Some(300); 50];
    samples.extend(vec![Some(0); 1000]);

    let (series, _start, end, _min, _deficit, _a90, _s50, _s10, _b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!(!depleted);
    assert!((end - 20_000.0).abs() < 1.0);
    for &v in &series {
        assert!(v <= 20_000.0 + 0.01, "balance exceeded W': {v}");
    }
}

#[test]
fn null_samples_do_not_change_balance() {
    let samples = vec![Some(300), Some(300), None, Some(300)];
    let (series, _start, _end, _min, _deficit, _a90, _s50, _s10, _b10, _depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert_eq!(series.len(), 4);
    assert!((series[0] - 19_950.0).abs() < 0.1);
    assert!((series[1] - 19_900.0).abs() < 0.1);
    assert!((series[2] - 19_900.0).abs() < 0.1);
    assert!((series[3] - 19_850.0).abs() < 0.1);
}

#[test]
fn empty_samples_returns_empty_series() {
    let samples: Vec<Option<i32>> = Vec::new();
    let (series, start, end, min, deficit, a90, s50, s10, b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!(series.is_empty());
    assert!((start - 20_000.0).abs() < 0.1);
    assert!((end - 20_000.0).abs() < 0.1);
    assert!((min - 20_000.0).abs() < 0.1);
    assert!((deficit - 0.0).abs() < 0.1);
    assert_eq!(a90, 0);
    assert_eq!(s50, 0);
    assert_eq!(s10, 0);
    assert_eq!(b10, 0);
    assert!(!depleted);
}

#[test]
fn time_buckets_are_mutually_exclusive_and_exhaustive() {
    let samples: Vec<Option<i32>> = vec![Some(300); 500];
    let (_series, _start, _end, _min, _deficit, a90, s50, s10, b10, depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!(depleted);
    assert_eq!(a90 + s50 + s10 + b10, 500);
    assert!(a90 > 0);
    assert!(s50 > 0);
    assert!(s10 > 0);
    assert!(b10 > 0);
}

#[test]
fn constant_power_exactly_at_cp_no_change() {
    let samples: Vec<Option<i32>> = vec![Some(250); 10];
    let (series, _start, end, _min, _deficit, _a90, _s50, _s10, _b10, _depleted) =
        compute_w_prime_balance(&samples, 250, 20_000);

    assert!((end - 20_000.0).abs() < 0.1);
    for &v in &series {
        assert!((v - 20_000.0).abs() < 0.1);
    }
}

// ---------------------------------------------------------------------------
// parse_args
// ---------------------------------------------------------------------------

#[test]
fn parse_args_valid_date() {
    let result = parse_args(r#"{"date":"2026-05-05"}"#).expect("valid args");
    assert_eq!(result.date, "2026-05-05");
    assert!(result.workout_id.is_none());
    assert!(result.cp_watts.is_none());
    assert!(result.w_prime_joules.is_none());
}

#[test]
fn parse_args_with_all_fields() {
    let result = parse_args(
        r#"{"date":"2026-05-05","workout_id":"abc","cp_watts":270,"w_prime_joules":22000}"#,
    )
    .expect("valid args");
    assert_eq!(result.date, "2026-05-05");
    assert_eq!(result.workout_id, Some("abc".to_string()));
    assert_eq!(result.cp_watts, Some(270));
    assert_eq!(result.w_prime_joules, Some(22000));
}

#[test]
fn parse_args_invalid_date() {
    let result = parse_args(r#"{"date":"not-a-date"}"#);
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("invalid date"));
}

#[test]
fn parse_args_invalid_json() {
    let result = parse_args("not json");
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("invalid arguments"));
}

#[test]
fn parse_args_rejects_non_positive_cp_watts() {
    let result = parse_args(r#"{"date":"2026-05-05","cp_watts":0}"#);
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("invalid cp_watts"));
}

#[test]
fn parse_args_rejects_non_positive_w_prime_joules() {
    let result = parse_args(r#"{"date":"2026-05-05","w_prime_joules":-1}"#);
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("invalid w_prime_joules"));
}

// ---------------------------------------------------------------------------
// preview_arguments
// ---------------------------------------------------------------------------

#[test]
fn preview_shows_date_only() {
    let tool = WPrimeBalance;
    let preview = tool.preview_arguments(r#"{"date":"2026-05-05"}"#);
    assert_eq!(preview, Some("date 2026-05-05".to_string()));
}

#[test]
fn preview_shows_workout_id() {
    let tool = WPrimeBalance;
    let preview = tool.preview_arguments(r#"{"date":"2026-05-05","workout_id":"abc123"}"#);
    assert_eq!(preview, Some("date 2026-05-05 workout abc123".to_string()));
}

#[test]
fn preview_shows_cp_and_w_prime() {
    let tool = WPrimeBalance;
    let preview =
        tool.preview_arguments(r#"{"date":"2026-05-05","cp_watts":270,"w_prime_joules":22000}"#);
    assert_eq!(
        preview,
        Some("date 2026-05-05 CP=270W W'=22000J".to_string())
    );
}

#[test]
fn preview_none_for_invalid_date() {
    let tool = WPrimeBalance;
    let preview = tool.preview_arguments(r#"{"date":"bad"}"#);
    assert_eq!(preview, None);
}

// ---------------------------------------------------------------------------
// select_workout
// ---------------------------------------------------------------------------

fn sample_workout(id: &str, date: &str) -> CompletedWorkout {
    CompletedWorkout::new(
        id.to_string(),
        "user-1".to_string(),
        format!("{date}T12:00:00"),
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics::default(),
        empty_details(),
        None,
    )
}

#[test]
fn select_workout_single_match() {
    let workouts = vec![sample_workout("w1", "2026-01-01")];
    let a = WPrimeBalanceArgs {
        date: "2026-01-01".to_string(),
        workout_id: None,
        cp_watts: None,
        w_prime_joules: None,
    };
    let result = select_workout(&a, workouts).unwrap();
    assert_eq!(result.completed_workout_id, "w1");
}

#[test]
fn select_workout_by_id() {
    let workouts = vec![
        sample_workout("w1", "2026-01-01"),
        sample_workout("w2", "2026-01-01"),
    ];
    let a = WPrimeBalanceArgs {
        date: "2026-01-01".to_string(),
        workout_id: Some("w2".to_string()),
        cp_watts: None,
        w_prime_joules: None,
    };
    let result = select_workout(&a, workouts).unwrap();
    assert_eq!(result.completed_workout_id, "w2");
}

#[test]
fn select_workout_not_found_by_id() {
    let workouts = vec![sample_workout("w1", "2026-01-01")];
    let a = WPrimeBalanceArgs {
        date: "2026-01-01".to_string(),
        workout_id: Some("nonexistent".to_string()),
        cp_watts: None,
        w_prime_joules: None,
    };
    let result = select_workout(&a, workouts);
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("not found"));
}

#[test]
fn select_workout_empty_list() {
    let a = WPrimeBalanceArgs {
        date: "2026-01-01".to_string(),
        workout_id: None,
        cp_watts: None,
        w_prime_joules: None,
    };
    let result = select_workout(&a, Vec::new());
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("no completed workouts"));
}

#[test]
fn select_workout_multiple_without_id() {
    let workouts = vec![
        sample_workout("w1", "2026-01-01"),
        sample_workout("w2", "2026-01-01"),
    ];
    let a = WPrimeBalanceArgs {
        date: "2026-01-01".to_string(),
        workout_id: None,
        cp_watts: None,
        w_prime_joules: None,
    };
    let result = select_workout(&a, workouts);
    assert!(result.is_err());
    assert!(result
        .err()
        .unwrap()
        .contains("multiple completed workouts"));
}

// ---------------------------------------------------------------------------
// LlmTool trait — is_available, name, prompt_guidance
// ---------------------------------------------------------------------------

#[test]
fn tool_name_is_get_w_prime_balance() {
    assert_eq!(WPrimeBalance.name(), "get_w_prime_balance");
}

#[test]
fn tool_available_when_data_port_present() {
    let mut ctx = test_context(None, None);
    ctx.data_port = Some(std::sync::Arc::new(NoopDataPort));
    assert!(WPrimeBalance.is_available(&ctx));
}

#[test]
fn tool_unavailable_when_data_port_absent() {
    let ctx = test_context(None, None);
    assert!(!WPrimeBalance.is_available(&ctx));
}

#[test]
fn prompt_guidance_includes_post_race() {
    let guidance = WPrimeBalance.prompt_guidance().unwrap();
    assert!(guidance.contains("post-race analysis"));
    assert!(guidance.contains("anaerobic capacity depletion"));
    assert!(guidance.contains("pacing strategy"));
}

#[test]
fn tool_definition_has_correct_schema() {
    let def = WPrimeBalance.definition();
    assert_eq!(def.name, "get_w_prime_balance");
    assert!(def.description.contains("W-prime"));
    assert!(def.description.contains("Skiba"));
    let schema: serde_json::Value = serde_json::from_str(&def.input_schema_json).unwrap();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "date");
}

// ---------------------------------------------------------------------------
// NoopDataPort for availability tests
// ---------------------------------------------------------------------------

struct NoopDataPort;

impl crate::domain::llm_tools::GetSelectedWorkoutDataPort for NoopDataPort {
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
        Box::pin(async { Ok(Vec::new()) })
    }

    fn load_selected_workout_data_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<
            crate::domain::llm_tools::get_selected_workout::SelectedWorkoutData,
            crate::domain::workout_summary::WorkoutSummaryError,
        >,
    > {
        Box::pin(async {
            Ok(
                crate::domain::llm_tools::get_selected_workout::SelectedWorkoutData {
                    completed: Vec::new(),
                    planned: Vec::new(),
                    races: Vec::new(),
                    summaries: Vec::new(),
                },
            )
        })
    }
}
