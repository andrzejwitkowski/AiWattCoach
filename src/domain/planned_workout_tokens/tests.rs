use super::{
    append_marker_to_description, build_planned_workout_match_token,
    extract_planned_workout_marker, format_planned_workout_marker,
};

#[test]
fn planned_workout_marker_round_trips() {
    let match_token = build_planned_workout_match_token("training-plan:user-1:w1:1:2026-03-26");
    let marker = format_planned_workout_marker(&match_token);

    assert_eq!(extract_planned_workout_marker(&marker), Some(match_token));
}

#[test]
fn append_marker_to_description_is_idempotent() {
    let marker = format_planned_workout_marker("ABC123EF45");
    let description = append_marker_to_description(Some("Build Session\n\n- 60m 70%"), &marker)
        .expect("marker description");

    assert_eq!(
        append_marker_to_description(Some(&description), &marker),
        Some(description)
    );
}

#[test]
fn extract_planned_workout_marker_rejects_non_hex_tokens() {
    assert_eq!(
        extract_planned_workout_marker("[AIWATTCOACH:pw=NOT-A-TOKEN]"),
        None
    );
    assert_eq!(
        extract_planned_workout_marker("[AIWATTCOACH:pw=ABC123EF4Z]"),
        None
    );
}

#[test]
fn extract_planned_workout_marker_rejects_wrong_length_tokens() {
    assert_eq!(
        extract_planned_workout_marker("[AIWATTCOACH:pw=ABC123]"),
        None
    );
    assert_eq!(
        extract_planned_workout_marker("[AIWATTCOACH:pw=ABC123EF4567]"),
        None
    );
}

#[test]
fn extract_planned_workout_marker_normalizes_lowercase_hex_tokens() {
    assert_eq!(
        extract_planned_workout_marker("[AIWATTCOACH:pw=abc123ef45]"),
        Some("ABC123EF45".to_string())
    );
}
