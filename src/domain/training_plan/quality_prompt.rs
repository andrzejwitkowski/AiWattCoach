use crate::domain::{
    llm::{build_chat_request, LlmChatMessage, LlmChatRequest, LlmChatRequestInput, LlmToolChoice},
    workout_summary::WorkoutRecap,
};

use super::TrainingPlanPlanningContext;

const PLAN_QUALITY_EVALUATOR_SYSTEM_PROMPT: &str = "You are an expert cycling coach evaluating a draft 14-day training plan. Score coaching quality from 1-10 using the rubric below. Return JSON only with shape {\"score\":1-10,\"critique\":\"...\",\"raise_to_next\":\"...\"}. No markdown, no tools, no extra keys.";

const PLAN_QUALITY_EVALUATOR_RUBRIC: &str = "\
Scoring rubric (use the full range; 7 is not a default):\n\
- 1-3: major rule violations or an unsafe/implausible load progression.\n\
- 4-5: usable but with structural gaps (race-week logic wrong, missing recovery, no specificity).\n\
- 6-7: sound, directionally correct plan; load logic plausible but only asserted, not demonstrated.\n\
- 8: sound AND every load/specificity claim is backed by the evidence block (verified TSB trajectory, race-day freshness, power-duration shape).\n\
- 9-10: reserved for plans where the evidence block shows both a defensible load trajectory and session-level specificity that matches the athlete's discipline and race priority, with no unaddressed gap.\n\
\n\
Anti-collapse: use the full 1-10 range. Reserve 8+ for drafts whose claims the evidence block confirms. If a draft satisfies every must-have and its load claims are verified, score it 8 or higher.\n\
\n\
In raise_to_next, state the single highest-leverage gap as one imperative sentence naming the concrete change that would move the draft up one band (actionable; do not restate the critique).";

const EVIDENCE_SECTION_MAX_CHARS: usize = 700;
const EVIDENCE_BLOCK_MAX_CHARS: usize = 2000;
const EVIDENCE_BLOCK_LABEL_OVERHEAD: usize = 120;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanQualityEvidence {
    pub load: Option<String>,
    pub power_curve: Option<String>,
    pub w_prime: Option<String>,
}

pub struct PlanQualityEvaluationInput<'a> {
    pub user_id: &'a str,
    pub workout_id: &'a str,
    pub saved_at_epoch_seconds: i64,
    pub workout_recap: &'a WorkoutRecap,
    pub planning_context: Option<&'a TrainingPlanPlanningContext>,
    pub draft_plan_text: &'a str,
    pub evidence: Option<&'a PlanQualityEvidence>,
}

pub fn plan_quality_evaluator_rubric() -> &'static str {
    PLAN_QUALITY_EVALUATOR_RUBRIC
}

pub fn format_plan_quality_evidence(evidence: Option<&PlanQualityEvidence>) -> String {
    let Some(evidence) = evidence else {
        return "Evidence: none (no tool output captured for this draft; do not award 8+ for load claims)."
            .to_string();
    };

    let mut missing = Vec::new();
    if evidence.load.is_none() {
        missing.push("simulate_forward_load");
    }
    if evidence.power_curve.is_none() {
        missing.push("selected_workout_power_curve");
    }
    if evidence.w_prime.is_none() {
        missing.push("get_w_prime_balance");
    }
    let missing_line = if missing.is_empty() {
        "none".to_string()
    } else {
        missing.join(", ")
    };

    // Budget fields so labels + missing + three sections always fit the block cap.
    let overhead = EVIDENCE_BLOCK_LABEL_OVERHEAD + missing_line.chars().count();
    let field_budget = EVIDENCE_BLOCK_MAX_CHARS.saturating_sub(overhead).max(3) / 3;
    let load = truncate_snippet(field_or_missing(evidence.load.as_deref()), field_budget);
    let power_curve = truncate_snippet(
        field_or_missing(evidence.power_curve.as_deref()),
        field_budget,
    );
    let w_prime = truncate_snippet(field_or_missing(evidence.w_prime.as_deref()), field_budget);

    format!(
        "Evidence (tool-verified facts; treat as authoritative):\n\
forward_load: {load}\n\
power_curve: {power_curve}\n\
w_prime: {w_prime}\n\
missing: {missing_line}"
    )
}

fn field_or_missing(value: Option<&str>) -> &str {
    value.unwrap_or("(not captured)")
}

pub fn assemble_plan_quality_evaluation_request(
    user_id: String,
    saved_at_epoch_seconds: i64,
    workout_recap: &WorkoutRecap,
    planning_context: Option<&TrainingPlanPlanningContext>,
    draft_plan_text: &str,
    evidence: Option<&PlanQualityEvidence>,
) -> LlmChatRequest {
    let planning_summary = planning_context_summary(planning_context);
    let recap_snippet = truncate_snippet(&workout_recap.text, 800);
    let evidence_block = format_plan_quality_evidence(evidence);
    let user_content = format!(
        "saved_at_epoch_seconds={saved_at_epoch_seconds}\n\nWorkout recap snippet:\n{recap_snippet}\n\nPlanning context summary:\n{planning_summary}\n\n{evidence_block}\n\nDraft plan:\n{draft_plan_text}\n\nReturn JSON only."
    );

    let mut request = build_chat_request(LlmChatRequestInput {
        user_id,
        system_prompt: format!(
            "{PLAN_QUALITY_EVALUATOR_SYSTEM_PROMPT}\n\n{}",
            plan_quality_evaluator_rubric()
        ),
        stable_context: String::new(),
        volatile_context: String::new(),
        conversation: vec![LlmChatMessage::user(user_content)],
        cache_scope_key: None,
        cache_key: None,
        reusable_cache_id: None,
    });
    request.tool_choice = LlmToolChoice::None;
    request
}

fn planning_context_summary(planning_context: Option<&TrainingPlanPlanningContext>) -> String {
    let Some(context) = planning_context else {
        return "(none)".to_string();
    };
    let rpe = context
        .rpe
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unset".to_string());
    let mut summary = format!(
        "rpe={rpe}; conversation_messages={}",
        context.messages.len()
    );
    if !context.messages.is_empty() {
        let recent = context
            .messages
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(|message| {
                format!(
                    "{}: {}",
                    message.role.as_str(),
                    truncate_snippet(&message.content, 240)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        summary = format!("{summary}\nRecent conversation:\n{recent}");
    }
    summary
}

pub(crate) fn truncate_evidence_section(text: &str) -> String {
    truncate_snippet(text, EVIDENCE_SECTION_MAX_CHARS)
}

fn truncate_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn append_raise_to_next(mut base: String, raise_to_next: &str) -> String {
    let raise = raise_to_next.trim();
    if !raise.is_empty() {
        base.push_str("\nraise_to_next: ");
        base.push_str(raise);
    }
    base
}

pub fn format_quality_feedback(score: u8, critique: &str, raise_to_next: &str) -> String {
    append_raise_to_next(
        format!(
            "Previous plan quality evaluation (must address):\nscore: {score}/10\ncritique: {critique}"
        ),
        raise_to_next,
    )
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

#[cfg(test)]
mod tests {
    use crate::domain::llm::{
        RACING_STRATEGIST_APP_EVIDENCE_CONTRACT, RACING_STRATEGIST_OPERATIONAL_GUIDELINES,
    };
    use crate::domain::workout_summary::WorkoutRecap;

    use super::{
        assemble_plan_quality_evaluation_request, format_plan_quality_evidence,
        format_quality_feedback, plan_quality_attempt_message,
        plan_quality_finished_accepted_message, plan_quality_finished_best_message,
        PlanQualityEvidence, EVIDENCE_BLOCK_MAX_CHARS, EVIDENCE_SECTION_MAX_CHARS,
    };

    fn sample_recap() -> WorkoutRecap {
        WorkoutRecap {
            text: "recap".to_string(),
            provider: "test".to_string(),
            model: "test".to_string(),
            generated_at_epoch_seconds: 1,
        }
    }

    #[test]
    fn quality_message_templates_match_locked_format() {
        assert_eq!(
            plan_quality_attempt_message(2, 5, 6, "Too much Z3.", ""),
            "Plan quality attempt 2/5: 6/10. Too much Z3."
        );
        assert_eq!(
            plan_quality_finished_accepted_message(8),
            "Plan quality finished: shipped 8/10 (accepted)."
        );
        assert_eq!(
            plan_quality_finished_best_message(6, 5),
            "Plan quality finished: shipped 6/10 (best after 5 attempts)."
        );
    }

    #[test]
    fn plan_quality_attempt_message_appends_raise_to_next_when_present() {
        assert_eq!(
            plan_quality_attempt_message(
                1,
                3,
                5,
                "Missing recovery.",
                "Add a rest day before race."
            ),
            "Plan quality attempt 1/3: 5/10. Missing recovery.\nraise_to_next: Add a rest day before race."
        );
    }

    #[test]
    fn format_quality_feedback_includes_raise_to_next_when_present() {
        assert_eq!(
            format_quality_feedback(6, "tempo heavy", "Cut midweek Z3."),
            "Previous plan quality evaluation (must address):\nscore: 6/10\ncritique: tempo heavy\nraise_to_next: Cut midweek Z3."
        );
        assert_eq!(
            format_quality_feedback(6, "tempo heavy", ""),
            "Previous plan quality evaluation (must address):\nscore: 6/10\ncritique: tempo heavy"
        );
    }

    #[test]
    fn evaluator_system_prompt_uses_rubric_not_generator_rulebook() {
        let request = assemble_plan_quality_evaluation_request(
            "u".to_string(),
            1,
            &sample_recap(),
            None,
            "draft",
            None,
        );
        let system = &request.system_prompt;
        assert!(system.contains("1-3:"));
        assert!(system.contains("4-5:"));
        assert!(system.contains("6-7:"));
        assert!(system.contains("use the full"));
        assert!(system.contains("raise_to_next"));
        assert!(!system.contains(RACING_STRATEGIST_APP_EVIDENCE_CONTRACT));
        assert!(!system.contains(RACING_STRATEGIST_OPERATIONAL_GUIDELINES));
    }

    #[test]
    fn evidence_none_and_partial_blocks_are_deterministic() {
        assert_eq!(
            format_plan_quality_evidence(None),
            "Evidence: none (no tool output captured for this draft; do not award 8+ for load claims)."
        );
        let partial = PlanQualityEvidence {
            load: Some("tsb_min=-12@2026-05-10".to_string()),
            power_curve: None,
            w_prime: None,
        };
        let rendered = format_plan_quality_evidence(Some(&partial));
        assert!(rendered.contains("forward_load: tsb_min=-12@2026-05-10"));
        assert!(rendered.contains("missing: selected_workout_power_curve, get_w_prime_balance"));
        assert!(rendered.starts_with("Evidence (tool-verified facts"));
    }

    #[test]
    fn evidence_block_keeps_all_sections_under_cap() {
        let long = "x".repeat(EVIDENCE_SECTION_MAX_CHARS);
        let evidence = PlanQualityEvidence {
            load: Some(long.clone()),
            power_curve: Some(long.clone()),
            w_prime: Some(long),
        };
        let rendered = format_plan_quality_evidence(Some(&evidence));
        assert!(rendered.chars().count() <= EVIDENCE_BLOCK_MAX_CHARS);
        assert!(rendered.contains("forward_load:"));
        assert!(rendered.contains("power_curve:"));
        assert!(rendered.contains("w_prime:"));
        assert!(rendered.contains("missing: none"));
        let w_prime_at = rendered.find("w_prime:").unwrap();
        let missing_at = rendered.find("missing: none").unwrap();
        assert!(w_prime_at < missing_at);
    }

    #[test]
    fn assemble_inserts_evidence_before_draft_plan() {
        let evidence = PlanQualityEvidence {
            load: Some("ok".to_string()),
            power_curve: Some("curve".to_string()),
            w_prime: None,
        };
        let request = assemble_plan_quality_evaluation_request(
            "u".to_string(),
            1,
            &sample_recap(),
            None,
            "MY_DRAFT",
            Some(&evidence),
        );
        let user = &request.conversation[0].content;
        let evidence_at = user.find("Evidence (tool-verified facts").unwrap();
        let draft_at = user.find("Draft plan:\nMY_DRAFT").unwrap();
        assert!(evidence_at < draft_at);
        assert!(user.contains("missing: get_w_prime_balance"));
    }
}
