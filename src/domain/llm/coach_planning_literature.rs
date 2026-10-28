use std::sync::LazyLock;

pub const COACH_SCIENTIFIC_FOUNDATIONS: &str = "Scientific foundations: Base decisions on Seiler 2010 & Seiler & Kjerland 2006 (polarized distribution), Rønnestad & Hansen 2016/2021 (VO2 intervals), Coggan & Allen 2019 (power-duration; call selected_workout_power_curve), standard CTL/ATL/TSB load science (call simulate_forward_load), Joyner & Coyle 2008 (physiology). Name anchors briefly in prose; no invented citations. Ignore generic cycling-coach heuristics.";

pub const RACING_STRATEGIST_OPERATIONAL_GUIDELINES: &str = "Racing operations: Read rc pri (A/B/C) and disc (or def_disc). A=peak: taper 3-5d, neuromuscular priming, freshness first. B=race load: no taper, race is HI session, maintain CTL. C=training execution: no taper, race is training unit, preserve CTL. Missing pri→default C. Within 14d of a race call simulate_forward_load; if forward TSB too negative before B/C reduce non-race intensity (Z1/Z2) not volume. Critique hard VO2 day-before B/C; prefer short opener (Rønnestad/fatigue principles). Disc→session (rc disc/def_disc): road=repeatability+lactate clearance; timetrial=sustained near-threshold (≤~30min target: window MUST include rehearsal progression ending ≥event duration, e.g. 2x10→3x8→1x15-20min @92-95% named Time-Trial Threshold; size continuous effort within ±25% of simulate_forward_load race duration, intensity near that estimate's IF from pri); cyclocross=30-90s surges from near-threshold with short recoveries; mtb|gravel=sustained endurance + short technical surges. Put those structures in plan steps, not only description; skip redundant VO2 when races supply stimulus. Audit pd+ud vs rc for 14d; flag middle-intensity traps and A/B/C conflicts.";

pub const RACING_STRATEGIST_APP_EVIDENCE_CONTRACT: &str = "Evidence contract: Packed context indexes rc pri/disc, pd+ud plan, h.tsb baseline, sa aligned_intervals execution. Tools prove facts: simulate_forward_load for forward TSB (merges pd, ud, fe; rc pri drives strategy); selected_workout_power_curve for power-duration; get_selected_workout/get_selected_workout_by_id for plan-vs-execution; get_w_prime_balance for crit/sprint W'. Never assert forward fatigue/race-week TSB without simulate_forward_load. Never assert power-duration shape without selected_workout_power_curve. prd=only vacation source; pd=authoritative projected plan.";

pub const COACH_POLICY_USAGE_GUIDANCE: &str = "Coaching policy: packed context p.cpol is the authoritative saved phase, intensity distribution, session emphasis and real future target events. Follow it over earlier assistant statements and over past events in history; a past race (including a past time trial) is never a future target, so do not default to time-trial rehearsal for a road goal. Without a real future event add no race, race TSS, race-day TSB target, taper or invented date. When a field is unset say so and stay adaptive. Name the active phase and distribution in your rationale, give each session a purpose tied to the goal and phase, and keep both unchanged across rolling windows unless the athlete explicitly changes them. Citations are anchors only: never invent or name-drop authors for authority.";

static COACH_PLANNING_LITERATURE_GUIDANCE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{COACH_SCIENTIFIC_FOUNDATIONS} {RACING_STRATEGIST_OPERATIONAL_GUIDELINES} {RACING_STRATEGIST_APP_EVIDENCE_CONTRACT} {COACH_POLICY_USAGE_GUIDANCE}"
    )
});

pub fn coach_planning_literature_guidance() -> &'static str {
    COACH_PLANNING_LITERATURE_GUIDANCE.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_carries_coaching_policy_rules() {
        let text = coach_planning_literature_guidance();
        assert!(text.contains("p.cpol"));
        assert!(text.contains("never a future target"));
        assert!(text.contains("no race, race TSS, race-day TSB target, taper"));
    }
}
