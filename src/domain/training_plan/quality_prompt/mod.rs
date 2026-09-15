mod availability;
mod feedback;
mod rubric;

pub use availability::format_plan_quality_availability;
pub use feedback::{
    draft_addresses_quality_checklist, format_quality_feedback, plan_quality_attempt_message,
    plan_quality_finished_accepted_message, plan_quality_finished_best_message,
    PLAN_QUALITY_PASS_SCORE,
};
pub use rubric::{
    assemble_plan_quality_evaluation_request, format_plan_quality_evidence,
    plan_quality_evaluator_rubric, PlanQualityEvaluationInput, PlanQualityEvidence,
};
pub(crate) use rubric::truncate_evidence_section;
#[cfg(test)]
pub(crate) use rubric::{EVIDENCE_BLOCK_MAX_CHARS, EVIDENCE_SECTION_MAX_CHARS};

#[cfg(test)]
mod tests {
    use crate::domain::llm::{
        RACING_STRATEGIST_APP_EVIDENCE_CONTRACT, RACING_STRATEGIST_OPERATIONAL_GUIDELINES,
    };
    use crate::domain::workout_summary::WorkoutRecap;

    use super::{
        assemble_plan_quality_evaluation_request, format_plan_quality_availability,
        format_plan_quality_evidence, format_quality_feedback, plan_quality_attempt_message,
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
            "Previous plan quality evaluation (must address):\nscore: 6/10\ncritique: tempo heavy\nUnresolved raise_to_next checklist (draft MUST include a section realizing each item; preferred heading \"Adjustment rules\"):\n1. Cut midweek Z3.\nIf any checklist item is omitted, the draft is invalid for shipping."
        );
        assert_eq!(
            format_quality_feedback(6, "tempo heavy", ""),
            "Previous plan quality evaluation (must address):\nscore: 6/10\ncritique: tempo heavy"
        );
    }

    #[test]
    fn draft_addresses_quality_checklist_requires_adjustment_rules_heading() {
        use super::draft_addresses_quality_checklist;
        assert!(draft_addresses_quality_checklist("any draft", ""));
        assert!(!draft_addresses_quality_checklist(
            "Plan with fatigue notes but no section",
            "Add explicit fatigue correction for 17.09 and 19.09"
        ));
        assert!(draft_addresses_quality_checklist(
            "## Adjustment rules\n- Cut load on 17.09 and 19.09 when TSB < -10",
            "Add explicit fatigue correction for 17.09 and 19.09"
        ));
    }

    #[test]
    fn evaluator_rubric_does_not_require_uncaptured_power_duration_for_band_8() {
        let rubric = super::plan_quality_evaluator_rubric();
        assert!(rubric.contains(
            "Require power-duration shape only when the evidence block captured power_curve/w_prime"
        ));
        assert!(rubric.contains("missing power tools alone is not a reason to stay at 7"));
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
            None,
        );
        let system = &request.system_prompt;
        assert!(system.contains("1-3:"));
        assert!(system.contains("4-5:"));
        assert!(system.contains("6-7:"));
        assert!(system.contains("use the full"));
        assert!(system.contains("Unavailable evidence is not a plan fault"));
        assert!(system.contains(
            "Draft-only claims of unavailable data without evidence-block support do not qualify"
        ));
        assert!(system.contains("raise_to_next"));
        assert!(!system.contains(RACING_STRATEGIST_APP_EVIDENCE_CONTRACT));
        assert!(!system.contains(RACING_STRATEGIST_OPERATIONAL_GUIDELINES));
    }

    #[test]
    fn evidence_none_and_partial_blocks_are_deterministic() {
        assert_eq!(
            format_plan_quality_evidence(None, None),
            "Evidence: none (no tool output captured for this draft; do not award 8+ for unverified load claims that tools never ran)."
        );
        let partial = PlanQualityEvidence {
            load: Some("tsb_min=-12@2026-05-10".to_string()),
            power_curve: None,
            w_prime: None,
        };
        let rendered = format_plan_quality_evidence(Some(&partial), None);
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
        let rendered = format_plan_quality_evidence(Some(&evidence), None);
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
            Some("availability: 2026-09-14 unavailable (Monday)"),
        );
        let user = &request.conversation[0].content;
        let evidence_at = user.find("Evidence (tool-verified facts").unwrap();
        let draft_at = user.find("Draft plan:\nMY_DRAFT").unwrap();
        assert!(evidence_at < draft_at);
        assert!(user.contains("missing: get_w_prime_balance"));
        assert!(user.contains("availability: 2026-09-14 unavailable (Monday)"));
    }

    #[test]
    fn format_plan_quality_availability_anchors_weekdays_to_dates() {
        use crate::domain::settings::Weekday;
        use crate::domain::training_context::WeeklyAvailabilityContext;

        let days = vec![
            WeeklyAvailabilityContext {
                weekday: Weekday::Mon,
                available: false,
                max_duration_minutes: None,
            },
            WeeklyAvailabilityContext {
                weekday: Weekday::Tue,
                available: true,
                max_duration_minutes: Some(90),
            },
        ];
        // 2026-09-13 is Sunday; next day is Monday 2026-09-14.
        let summary = format_plan_quality_availability("2026-09-13", true, &days, 2);
        assert_eq!(
            summary,
            "availability: 2026-09-14 unavailable (Monday); 2026-09-15 available max=90m (Tuesday)"
        );
        assert_eq!(
            format_plan_quality_availability("2026-09-13", false, &days, 2),
            "availability: not configured"
        );
    }
}
