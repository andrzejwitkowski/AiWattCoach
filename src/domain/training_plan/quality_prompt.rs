use crate::domain::{
    llm::{
        build_chat_request, coach_planning_literature_guidance, LlmChatMessage, LlmChatRequest,
        LlmChatRequestInput, LlmToolChoice,
    },
    workout_summary::WorkoutRecap,
};

use super::TrainingPlanPlanningContext;

const PLAN_QUALITY_EVALUATOR_SYSTEM_PROMPT: &str = "You are an expert cycling coach evaluating a draft 14-day training plan. Score coaching quality from 1-10 using the scientific foundations below. Return JSON only with shape {\"score\":1-10,\"critique\":\"...\"}. No markdown, no tools, no extra keys.";

pub fn assemble_plan_quality_evaluation_request(
    user_id: String,
    saved_at_epoch_seconds: i64,
    workout_recap: &WorkoutRecap,
    planning_context: Option<&TrainingPlanPlanningContext>,
    draft_plan_text: &str,
) -> LlmChatRequest {
    let planning_summary = planning_context_summary(planning_context);
    let recap_snippet = truncate_snippet(&workout_recap.text, 800);
    let user_content = format!(
        "saved_at_epoch_seconds={saved_at_epoch_seconds}\n\nWorkout recap snippet:\n{recap_snippet}\n\nPlanning context summary:\n{planning_summary}\n\nDraft plan:\n{draft_plan_text}\n\nReturn JSON only."
    );

    let mut request = build_chat_request(LlmChatRequestInput {
        user_id,
        system_prompt: format!(
            "{PLAN_QUALITY_EVALUATOR_SYSTEM_PROMPT}\n\n{}",
            coach_planning_literature_guidance()
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

fn truncate_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(max_chars).collect();
    format!("{truncated}...")
}

pub fn format_quality_feedback(score: u8, critique: &str) -> String {
    format!(
        "Previous plan quality evaluation (must address):\nscore: {score}/10\ncritique: {critique}"
    )
}

pub fn plan_quality_attempt_message(
    attempt: u32,
    max_loops: u32,
    score: u8,
    critique: &str,
) -> String {
    format!("Plan quality attempt {attempt}/{max_loops}: {score}/10. {critique}")
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
    use super::{
        plan_quality_attempt_message, plan_quality_finished_accepted_message,
        plan_quality_finished_best_message,
    };

    #[test]
    fn quality_message_templates_match_locked_format() {
        assert_eq!(
            plan_quality_attempt_message(2, 5, 6, "Too much Z3."),
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
}
