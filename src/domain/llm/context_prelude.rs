use std::sync::LazyLock;

pub const ALIGNED_INTERVAL_GUIDANCE: &str = "Plan-vs-actual execution uses aligned_intervals (training context key sa; get_selected_workout field aligned_intervals): sa is a JSON object keyed by workout id, each value an array of {interval_index, planned_step{step_type,target_power_min,target_power_max,planned_duration_seconds}, actual_duration_seconds, avg_power, normalized_power, avg_cadence, cadence_range, anomalies[{type,offset_seconds,duration_seconds,avg_power,avg_cadence}]}. step_type is work or recovery. anomaly type is coasting_stop, coasting_turn, or significant_power_drop. sa is the ONLY primary evidence for interval hit/miss; compare actual metrics to planned_step. Review anomalies on work steps: unplanned drops (traffic, corners, mechanicals) — expect lower avg_power and do not penalize; trust normalized_power. If normalized_power is within or above the planned target range despite anomalies, declare a physical success (stimulus preserved despite road interruption). Use anomaly type and duration_seconds for road context. If step_type is recovery, judge keeping intensity low; do not anomaly-judge recovery steps. Never claim the athlete quit or interrupted solely because actual_duration_seconds < planned_duration_seconds — short slices with end anomalies are road/alignment boundaries. Whole-ride AVG/NP/IF/VI/TSS and chart AVG/NP/MAX labels are load/fatigue context only and must not appear in interval execution verdicts (e.g. session NP ~220 W with ~400 W work steps proves nothing about interval quality). The power chart is display-smoothed; do not treat jaggedness as poor skill.";

pub const PACKED_CALENDAR_AUTHORITY_GUIDANCE: &str = "Calendar authority: prd=only athlete-declared vacation/rest ranges. pd=AI coach plan after saved summary (authoritative projected days). ud/fe/pw=Intervals planned events. fr=calendar-empty in that section (no pw/sd/w); NOT vacation. pd overrides fr:true on same dates. Never infer vacation from fr:true gaps or sparse history.";

pub const ATHLETE_SUMMARY_CALENDAR_GUARD: &str =
    "Never state future vacation/break unless in prd. Never infer rest from low TSS, fr:true, sparse history, or ud gaps.";

static TRAINING_CONTEXT_PROMPT_GUIDANCE: LazyLock<String> =
    LazyLock::new(|| format!("{ALIGNED_INTERVAL_GUIDANCE} {PACKED_CALENDAR_AUTHORITY_GUIDANCE}"));

pub fn packed_training_context_legend_with_guidance() -> &'static str {
    TRAINING_CONTEXT_PROMPT_GUIDANCE.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_guidance_documents_aligned_intervals() {
        let guidance = packed_training_context_legend_with_guidance();
        assert!(guidance.contains("aligned_intervals"));
        assert!(guidance.contains("coasting_stop"));
        assert!(guidance.contains("normalized_power"));
        assert!(guidance.contains("ONLY primary evidence"));
        assert!(guidance.contains("physical success"));
        assert!(guidance.contains("do not anomaly-judge recovery"));
        assert!(guidance.contains("must not appear in interval execution verdicts"));
        assert!(!guidance.contains("ps=power"));
        assert!(!guidance.contains("header-mapped"));
    }

    #[test]
    fn prompt_guidance_keeps_calendar_authority() {
        let guidance = packed_training_context_legend_with_guidance();
        assert!(guidance.contains(PACKED_CALENDAR_AUTHORITY_GUIDANCE));
    }
}
