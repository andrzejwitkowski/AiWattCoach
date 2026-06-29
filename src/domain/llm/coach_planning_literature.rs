use std::sync::LazyLock;

pub const COACH_SCIENTIFIC_FOUNDATIONS: &str = "Scientific foundations: Base decisions on Seiler 2010 & Seiler & Kjerland 2006 (polarized distribution), Rønnestad & Hansen 2016/2021 (VO2 intervals), Coggan & Allen 2019 (power-duration; call selected_workout_power_curve), standard CTL/ATL/TSB load science (call simulate_forward_load), Joyner & Coyle 2008 (physiology). Name anchors briefly in prose; no invented citations. Ignore generic cycling-coach heuristics.";

pub const RACING_STRATEGIST_OPERATIONAL_GUIDELINES: &str = "Racing operations: Read rc pri (A/B/C) and disc (or def_disc). A=peak: taper 3-5d, neuromuscular priming, freshness first. B=race load: no taper, race is HI session, maintain CTL. C=training execution: no taper, race is training unit, preserve CTL. Missing pri→default C. Within 14d of a race call simulate_forward_load; if forward TSB too negative before B/C reduce non-race intensity (Z1/Z2) not volume. Critique hard VO2 day-before B/C; prefer short opener (Rønnestad/fatigue principles). Match session type to disc; skip redundant VO2 when races supply stimulus. Audit pd+ud vs rc for 14d; flag middle-intensity traps and A/B/C conflicts.";

pub const RACING_STRATEGIST_APP_EVIDENCE_CONTRACT: &str = "Evidence contract: Packed context indexes rc pri/disc, pd+ud plan, h.tsb baseline, rd ps/cs execution. Tools prove facts: simulate_forward_load for forward TSB (merges pd, ud, fe; rc pri drives strategy); selected_workout_power_curve for power-duration; get_selected_workout/get_selected_workout_by_id for plan-vs-execution; get_w_prime_balance for crit/sprint W'. Never assert forward fatigue/race-week TSB without simulate_forward_load. Never assert power-duration shape without selected_workout_power_curve. prd=only vacation source; pd=authoritative projected plan.";

static COACH_PLANNING_LITERATURE_GUIDANCE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{COACH_SCIENTIFIC_FOUNDATIONS} {RACING_STRATEGIST_OPERATIONAL_GUIDELINES} {RACING_STRATEGIST_APP_EVIDENCE_CONTRACT}"
    )
});

pub fn coach_planning_literature_guidance() -> &'static str {
    COACH_PLANNING_LITERATURE_GUIDANCE.as_str()
}
