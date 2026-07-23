use crate::domain::llm::{coach_planning_literature_guidance, COACH_SCIENTIFIC_FOUNDATIONS};

pub const RACING_STRATEGIST_LITERATURE_ANCHORS: &str = COACH_SCIENTIFIC_FOUNDATIONS;

pub use crate::domain::llm::{
    RACING_STRATEGIST_APP_EVIDENCE_CONTRACT, RACING_STRATEGIST_OPERATIONAL_GUIDELINES,
};

pub fn racing_strategist_plan_guidance() -> String {
    format!(
        "{} Before finalizing the plan window, cross-check pd and ud against rc for the next 14 days. Before returning the plan envelope, call simulate_forward_load at least once with no arguments to audit the current schedule, then with dated_workout_text containing your draft plan to validate forward TSB under A/B/C rules. Put literature-grounded racing audit rationale in the JSON description field only; keep the plan field as workout-builder syntax only.",
        coach_planning_literature_guidance()
    )
}

pub fn racing_strategist_calendar_guidance() -> String {
    format!(
        "{} When the athlete asks about plan, races, taper, or the next 14 days, produce a critical audit in prose: races from rc (pri, disc/def_disc, dates); pd/ud conflicts vs A/B/C intent; middle-intensity traps; tool-backed TSB trend from simulate_forward_load; power-duration facts from selected_workout_power_curve when relevant; recommended physiological focus for the block. Cite literature anchors briefly. Calendar chat cannot rewrite pd; update_planned_workout may adjust Intervals ud workouts only; AI plan regeneration requires saving a workout summary.",
        coach_planning_literature_guidance()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literature_anchors_name_canonical_sources() {
        let anchors = RACING_STRATEGIST_LITERATURE_ANCHORS;
        assert!(anchors.contains("Seiler 2010"));
        assert!(anchors.contains("Seiler & Kjerland 2006"));
        assert!(anchors.contains("Rønnestad"));
        assert!(anchors.contains("Coggan & Allen"));
        assert!(anchors.contains("Joyner & Coyle"));
        assert!(anchors.contains("generic cycling-coach heuristics"));
    }

    #[test]
    fn operational_guidelines_cover_race_priorities() {
        let ops = RACING_STRATEGIST_OPERATIONAL_GUIDELINES;
        assert!(ops.contains("A=peak"));
        assert!(ops.contains("B=race load"));
        assert!(ops.contains("C=training execution"));
        assert!(ops.contains("rc pri"));
        assert!(ops.contains("middle-intensity traps"));
    }

    #[test]
    fn app_evidence_contract_requires_tools() {
        let contract = RACING_STRATEGIST_APP_EVIDENCE_CONTRACT;
        assert!(contract.contains("simulate_forward_load"));
        assert!(contract.contains("selected_workout_power_curve"));
        assert!(contract.contains("get_selected_workout"));
        assert!(contract.contains("get_w_prime_balance"));
    }

    #[test]
    fn plan_guidance_puts_rationale_in_description() {
        let guidance = racing_strategist_plan_guidance();
        assert!(guidance.contains("description field"));
        assert!(guidance.contains("simulate_forward_load"));
        assert!(guidance.contains("Seiler 2010"));
    }

    #[test]
    fn calendar_guidance_requires_audit_workflow() {
        let guidance = racing_strategist_calendar_guidance();
        assert!(guidance.contains("critical audit"));
        assert!(guidance.contains("simulate_forward_load"));
        assert!(guidance.contains("cannot rewrite pd"));
    }
}
