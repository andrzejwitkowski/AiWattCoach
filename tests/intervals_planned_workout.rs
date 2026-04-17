use aiwattcoach::domain::intervals::{
    parse_planned_workout, parse_planned_workout_days, serialize_planned_workout, PlannedWorkout,
    PlannedWorkoutDays, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
    PlannedWorkoutTarget,
};

#[test]
fn parses_single_planned_workout_with_titles_repeat_ramp_steps_and_cooldown() {
    let parsed = parse_planned_workout(
        "Warmup\n- 15m ramp 120-160W\nMain Set 4x\n- 5m 95%\n- 2m 55%\nCooldown\n- 10m 50%",
    )
    .expect("planned workout should parse");

    assert_eq!(parsed.lines.len(), 7);
    assert_eq!(parsed.lines[0].text(), Some("Warmup"));
    assert_eq!(
        parsed.lines[1].step().map(|step| step.duration_seconds),
        Some(900)
    );
    assert_eq!(
        parsed.lines[1].step().map(|step| &step.kind),
        Some(&PlannedWorkoutStepKind::Ramp)
    );
    assert_eq!(
        parsed.lines[1].step().map(|step| &step.target),
        Some(&PlannedWorkoutTarget::WattsRange { min: 120, max: 160 })
    );
    assert_eq!(parsed.lines[2].repeat().map(|repeat| repeat.count), Some(4));
    assert_eq!(
        parsed.lines[2]
            .repeat()
            .and_then(|repeat| repeat.title.as_deref()),
        Some("Main Set")
    );
    assert_eq!(
        parsed.lines[3].step().map(|step| step.duration_seconds),
        Some(300)
    );
    assert_eq!(
        parsed.lines[3].step().map(|step| &step.target),
        Some(&PlannedWorkoutTarget::PercentFtp {
            min: 95.0,
            max: 95.0
        })
    );
    assert_eq!(
        parsed.lines[4].step().map(|step| step.duration_seconds),
        Some(120)
    );
    assert_eq!(parsed.lines[5].text(), Some("Cooldown"));
    assert_eq!(
        parsed.lines[6].step().map(|step| step.duration_seconds),
        Some(600)
    );

    assert_eq!(
        serialize_planned_workout(&parsed),
        "Warmup\n- 15m ramp 120-160W\nMain Set 4x\n- 5m 95%\n- 2m 55%\nCooldown\n- 10m 50%"
    );
}

#[test]
fn parses_dated_outer_format_with_multiple_days_and_rest_day() {
    let parsed = parse_planned_workout_days(
        "2026-04-01\nWarmup\n- 15m ramp 120-160W\n2026-04-02\nrest day\n2026-04-03\nTempo\n- 20m 88-92%",
    )
    .expect("planned workout days should parse");

    assert_eq!(parsed.days.len(), 3);
    assert_eq!(parsed.days[0].date, "2026-04-01");
    assert!(parsed.days[0].planned_workout().is_some());
    assert_eq!(parsed.days[1].date, "2026-04-02");
    assert!(parsed.days[1].is_rest_day());
    assert_eq!(parsed.days[1].rest_day_reason(), None);
    assert!(parsed.days[1].planned_workout().is_none());
    assert_eq!(parsed.days[2].date, "2026-04-03");
    assert_eq!(
        parsed.days[2]
            .planned_workout()
            .map(|workout| workout.lines.len()),
        Some(2)
    );
}

#[test]
fn parses_rest_day_with_reason() {
    let parsed =
        parse_planned_workout_days("2026-04-02\nRest Day: accumulated fatigue after race block")
            .expect("planned workout days should parse");

    assert_eq!(parsed.days.len(), 1);
    assert!(parsed.days[0].is_rest_day());
    assert_eq!(
        parsed.days[0].rest_day_reason(),
        Some("accumulated fatigue after race block")
    );
    assert!(parsed.days[0].planned_workout().is_none());
}

#[test]
fn includes_failing_date_in_invalid_day_parse_errors() {
    let error = parse_planned_workout_days("2026-04-01\n- 15m 60%\n2026-04-02\n- not-a-step")
        .expect_err("invalid day should fail");

    assert!(
        error.to_string().contains("2026-04-02"),
        "unexpected error: {error}"
    );
}

#[test]
fn serializes_parsed_workout_to_canonical_normalized_string() {
    let parsed = parse_planned_workout(
        "  Warmup  \n - 15min   ramp 120-160W \n Main Set   4x \n - 5min   95% \n - 2min 55% \n Cooldown \n - 10min 50% ",
    )
    .expect("planned workout should parse");

    assert_eq!(
        serialize_planned_workout(&parsed),
        "Warmup\n- 15m ramp 120-160W\nMain Set 4x\n- 5m 95%\n- 2m 55%\nCooldown\n- 10m 50%"
    );
}

#[test]
fn rejects_step_lines_with_unsupported_trailing_tokens() {
    let error = parse_planned_workout("- 15m 95% extra").expect_err("step should fail");

    assert!(
        error
            .to_string()
            .contains("invalid planned workout step: - 15m 95% extra"),
        "unexpected error: {error}"
    );
}

#[test]
fn does_not_treat_plain_text_ending_with_x_as_repeat_header() {
    let parsed = parse_planned_workout("Spinx").expect("text should parse");

    assert_eq!(parsed.lines.len(), 1);
    assert_eq!(parsed.lines[0].text(), Some("Spinx"));
    assert!(parsed.lines[0].repeat().is_none());
}

#[test]
fn rejects_non_empty_content_before_first_date_header() {
    let error =
        parse_planned_workout_days("Warmup\n2026-04-01\n- 15m 60%").expect_err("should fail");

    assert!(
        error
            .to_string()
            .contains("content before first date header"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_date_header_with_no_body() {
    let error = parse_planned_workout_days("2026-04-01").expect_err("should fail");

    assert!(
        error.to_string().contains("2026-04-01"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_invalid_calendar_date_headers() {
    let error = parse_planned_workout_days("2026-13-99\nrest day").expect_err("should fail");

    assert!(
        error
            .to_string()
            .contains("content before first date header"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_empty_input_without_any_date_headers() {
    let error = parse_planned_workout_days("").expect_err("should fail");

    assert!(
        error.to_string().contains("no date headers found"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_whitespace_only_input_without_any_date_headers() {
    let error = parse_planned_workout_days("   \n\n  ").expect_err("should fail");

    assert!(
        error.to_string().contains("no date headers found"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_non_positive_target_values() {
    let percent_error = parse_planned_workout("- 15m 0%").expect_err("percent should fail");
    let watts_error = parse_planned_workout("- 15m ramp 0-120W").expect_err("watts should fail");

    assert!(
        percent_error
            .to_string()
            .contains("invalid planned workout step"),
        "unexpected error: {percent_error}"
    );
    assert!(
        watts_error
            .to_string()
            .contains("invalid planned workout step"),
        "unexpected error: {watts_error}"
    );
}

#[test]
fn parses_watts_targets_with_lowercase_w_suffix() {
    let parsed =
        parse_planned_workout("- 15m ramp 120-160w").expect("planned workout should parse");

    assert_eq!(parsed.lines.len(), 1);
    assert_eq!(
        parsed.lines[0].step().map(|step| &step.target),
        Some(&PlannedWorkoutTarget::WattsRange { min: 120, max: 160 })
    );
}

#[test]
fn public_intervals_surface_exports_planned_workout_model_types() {
    let workout: PlannedWorkout =
        parse_planned_workout("Warmup\n- 5m 60%").expect("planned workout should parse");
    let days: PlannedWorkoutDays = parse_planned_workout_days("2026-04-01\nWarmup\n- 5m 60%")
        .expect("planned workout days should parse");
    let first_line: &PlannedWorkoutLine = &workout.lines[0];
    let first_step: &PlannedWorkoutStep = workout.lines[1].step().expect("step line");

    assert_eq!(first_line.text(), Some("Warmup"));
    assert_eq!(first_step.kind, PlannedWorkoutStepKind::Steady);
    assert_eq!(days.days.len(), 1);
}
