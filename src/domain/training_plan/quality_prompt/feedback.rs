fn append_raise_to_next(mut base: String, raise_to_next: &str) -> String {
    let raise = raise_to_next.trim();
    if !raise.is_empty() {
        base.push_str("\nraise_to_next: ");
        base.push_str(raise);
    }
    base
}

pub fn format_quality_feedback(score: u8, critique: &str, raise_to_next: &str) -> String {
    let mut feedback = format!(
        "Previous plan quality evaluation (must address):\nscore: {score}/10\ncritique: {critique}"
    );
    let raise = raise_to_next.trim();
    if !raise.is_empty() {
        feedback.push_str(
            "\nUnresolved raise_to_next checklist (JSON `description` MUST include a section realizing each item; preferred heading \"Adjustment rules\"):\n1. ",
        );
        feedback.push_str(raise);
        feedback.push_str(
            "\nPut Adjustment rules only in `description`, never in `plan`. If any checklist item is omitted from description, the draft is invalid for shipping.",
        );
    }
    feedback
}

/// Binding gate for quality replan: empty raise always passes; otherwise the draft
/// `description` must contain an `Adjustment rules` section heading (case-insensitive).
pub fn draft_addresses_quality_checklist(
    draft_description: Option<&str>,
    raise_to_next: &str,
) -> bool {
    if raise_to_next.trim().is_empty() {
        return true;
    }
    draft_description
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("adjustment rules")
}

pub fn plan_quality_attempt_message(
    attempt: u32,
    max_loops: u32,
    score: u8,
    critique: &str,
    raise_to_next: &str,
) -> String {
    append_raise_to_next(
        format!("Plan quality attempt {attempt}/{max_loops}: {score}/10. {critique}"),
        raise_to_next,
    )
}

pub fn plan_quality_finished_accepted_message(score: u8) -> String {
    format!("Plan quality finished: shipped {score}/10 (accepted).")
}

pub fn plan_quality_finished_best_message(score: u8, max_loops: u32) -> String {
    format!("Plan quality finished: shipped {score}/10 (best after {max_loops} attempts).")
}

pub const PLAN_QUALITY_PASS_SCORE: u8 = 7;
