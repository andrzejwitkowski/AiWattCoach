use crate::domain::{
    llm::{build_chat_request, LlmChatMessage, LlmChatRequest, LlmChatRequestInput, LlmToolChoice},
    workout_summary::WorkoutRecap,
};

use super::super::TrainingPlanPlanningContext;

const PLAN_QUALITY_EVALUATOR_SYSTEM_PROMPT: &str = "You are an expert cycling coach evaluating a draft 14-day training plan. Score coaching quality from 1-10 using the rubric below. Return JSON only with shape {\"score\":1-10,\"critique\":\"...\",\"raise_to_next\":\"...\"}. No markdown, no tools, no extra keys.";

const PLAN_QUALITY_EVALUATOR_RUBRIC: &str = "\
Scoring rubric (use the full range; 7 is not a default):\n\
- 1-3: major rule violations or an unsafe/implausible load progression.\n\
- 4-5: usable but with structural gaps (race-week logic wrong, missing recovery, no specificity).\n\
- 6-7: sound, directionally correct plan; load logic plausible but only asserted, not demonstrated.\n\
- 8: sound AND load claims are backed by the evidence block (verified TSB trajectory and race-day freshness, including labeled estimated race TSS). Require power-duration shape only when the evidence block captured power_curve/w_prime or the draft asserts that shape; missing power tools alone is not a reason to stay at 7.\n\
- 9-10: reserved for plans where the evidence block shows both a defensible load trajectory and session-level specificity that matches the athlete's discipline and race priority, with no unaddressed gap.\n\
\n\
Anti-collapse: use the full 1-10 range. Reserve 8+ for drafts whose claims the evidence block confirms. If a draft satisfies every must-have and its load claims are verified, score it 8 or higher.\n\
\n\
Unavailable evidence is not a plan fault: If the evidence block states that a datum is unavailable and gives the reason (no race TSS, no power samples / insufficient data, no executed intervals), do NOT deduct for the missing datum. Score how the plan handles that uncertainty: conservative targets with a stated rationale are acceptable, and such a draft can score 8 or higher. Draft-only claims of unavailable data without evidence-block support do not qualify.\n\
\n\
Treat availability lines in the evidence block as authoritative constraints. A rest day on an unavailable weekday is not a conflict with the recap.\n\
\n\
In raise_to_next, state the single highest-leverage gap as one imperative sentence naming the concrete change that would move the draft up one band (actionable; do not restate the critique).";

pub(crate) const EVIDENCE_SECTION_MAX_CHARS: usize = 700;
pub(crate) const EVIDENCE_BLOCK_MAX_CHARS: usize = 2000;
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
    /// Date-anchored availability constraints for the evaluation window.
    pub availability_summary: Option<&'a str>,
}

pub fn plan_quality_evaluator_rubric() -> &'static str {
    PLAN_QUALITY_EVALUATOR_RUBRIC
}

pub fn format_plan_quality_evidence(
    evidence: Option<&PlanQualityEvidence>,
    availability_summary: Option<&str>,
) -> String {
    let availability_line = match availability_summary
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(summary) => format!("\n{summary}"),
        None => String::new(),
    };

    let Some(evidence) = evidence else {
        return format!(
            "Evidence: none (no tool output captured for this draft; do not award 8+ for unverified load claims that tools never ran).{availability_line}"
        );
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
    let overhead = EVIDENCE_BLOCK_LABEL_OVERHEAD
        + missing_line.chars().count()
        + availability_line.chars().count();
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
missing: {missing_line}{availability_line}"
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
    availability_summary: Option<&str>,
) -> LlmChatRequest {
    let planning_summary = planning_context_summary(planning_context);
    let recap_snippet = truncate_snippet(&workout_recap.text, 800);
    let evidence_block = format_plan_quality_evidence(evidence, availability_summary);
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

pub(crate) fn truncate_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(max_chars).collect();
    format!("{truncated}...")
}
