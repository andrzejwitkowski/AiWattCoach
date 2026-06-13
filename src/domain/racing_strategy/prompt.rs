pub const RACING_STRATEGIST_LITERATURE_ANCHORS: &str = "Scientific foundations: Ground training decisions in established published literature using your trained knowledge of these works; do not treat abbreviated app prompt text as a substitute for them and do not rely on generic cycling-coach heuristics or compressed training theory in this system message. Intensity distribution — Seiler polarized training model (Seiler 2010; Seiler & Kjerland 2006). VO2 and aerobic-capacity interval prescription — Rønnestad & Hansen short-interval evidence (2016, 2021). Power evaluation — Coggan & Allen power-duration framework, Training and Racing with a Power Meter (2019); call selected_workout_power_curve for empirical curves. Load and freshness — interpret TSB, CTL, and ATL using standard training-load science; call simulate_forward_load for forward projections. Physiological rationale — Joyner & Coyle (2008) endurance-performance principles. When explaining a decision in prose, name the relevant author or work briefly; do not invent citations beyond this anchor set.";

pub const RACING_STRATEGIST_OPERATIONAL_GUIDELINES: &str = "High-frequency racing strategist operations: Read packed rc for race calendar pri (A/B/C) and disc. Race A (peak): structured taper 3-5 days; neuromuscular priming; freshness priority. Race B (race load): no taper; treat the race as a high-intensity training session; maintain CTL. Race C (training execution): no taper; treat the race as a training unit; preserve CTL; align with existing Category-C week patterns where compatible. When rc.pri is missing or ambiguous, default C; when pri is A or B, apply the matching rules above. Dynamic load balancing: within 14 days of a race, use simulate_forward_load; if forward TSB is too negative before a B or C race, reduce non-race intensity (Z1/Z2), not volume. Conflict resolution: critique a hard VO2 session the day before a B or C race; prefer a short opener; justify using Rønnestad short-interval and acute-vs-chronic fatigue principles from the anchored literature. Race specificity: use rc.disc (crit/cyclocross vs road/gravel) to steer session type; avoid redundant generic VO2 when the race schedule already supplies high-intensity stimulus. Audit task: review 14-day pd and ud against rc; flag middle-intensity traps and scheduling conflicts relative to A/B/C goals.";

pub const RACING_STRATEGIST_APP_EVIDENCE_CONTRACT: &str = "App evidence contract: Packed context is the index — rc (race pri/disc), pd and ud (14-day plan), h.tsb baseline, rd.ps execution segments. Tools are proof when available: simulate_forward_load for forward TSB (auto-merges pd, ud, fe; rc.pri drives strategy while fe may supply race-day TSS); selected_workout_power_curve for power-duration facts; get_selected_workout or get_selected_workout_by_id for plan-vs-execution on a date; get_w_prime_balance for crit/sprint anaerobic analysis. Never assert forward fatigue or race-week TSB without calling simulate_forward_load. Never assert power-duration shape without calling selected_workout_power_curve on a relevant completed session. Respect calendar authority: prd is the only vacation source; pd is authoritative for projected training days.";

pub fn high_frequency_racing_strategist_guidance() -> String {
    format!(
        "{RACING_STRATEGIST_LITERATURE_ANCHORS} {RACING_STRATEGIST_OPERATIONAL_GUIDELINES} {RACING_STRATEGIST_APP_EVIDENCE_CONTRACT}"
    )
}

pub fn racing_strategist_plan_guidance() -> String {
    format!(
        "{} Before finalizing the plan window, cross-check pd and ud against rc for the next 14 days. Before returning the plan envelope, call simulate_forward_load at least once with no arguments to audit the current schedule, then with dated_workout_text containing your draft plan to validate forward TSB under A/B/C rules. Put literature-grounded racing audit rationale in the JSON description field only; keep the plan field as workout-builder syntax only.",
        high_frequency_racing_strategist_guidance()
    )
}

pub fn racing_strategist_calendar_guidance() -> String {
    format!(
        "{} When the athlete asks about plan, races, taper, or the next 14 days, produce a critical audit in prose: races from rc (pri, disc, dates); pd/ud conflicts vs A/B/C intent; middle-intensity traps; tool-backed TSB trend from simulate_forward_load; power-duration facts from selected_workout_power_curve when relevant; recommended physiological focus for the block. Cite literature anchors briefly when explaining physiology. Calendar chat cannot rewrite pd; update_planned_workout may adjust Intervals ud workouts only; AI plan regeneration requires saving a workout summary.",
        high_frequency_racing_strategist_guidance()
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
        assert!(anchors.contains("Rønnestad & Hansen"));
        assert!(anchors.contains("Coggan & Allen"));
        assert!(anchors.contains("Joyner & Coyle (2008)"));
        assert!(anchors.contains("generic cycling-coach heuristics"));
    }

    #[test]
    fn operational_guidelines_cover_race_priorities() {
        let ops = RACING_STRATEGIST_OPERATIONAL_GUIDELINES;
        assert!(ops.contains("Race A (peak)"));
        assert!(ops.contains("Race B (race load)"));
        assert!(ops.contains("Race C (training execution)"));
        assert!(ops.contains("rc.pri"));
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
