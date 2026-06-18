pub const TRAINING_PLAN_WINDOW_DAY_COUNT: usize = 14;

use crate::domain::racing_strategy::racing_strategist_plan_guidance;

const TRAINING_PLAN_OUTPUT_GRAMMAR: &str = "Critical rules: Output ONLY valid JSON matching this schema. Your full response is parsed directly as JSON by the application. Any text outside the JSON object will be treated as an invalid response. Do not include markdown fences or any extra text outside the JSON object. Put the workout-builder text only in the `plan` field. Put any coach commentary only in the optional `description` field. Apply every workout-builder grammar rule specifically to the `plan` field value. Every actionable workout step in `plan` MUST begin with a hyphen followed by a space (`- `). Do not invent syntax. Output grammar for the `plan` field: One dated section per day. Start each section with a YYYY-MM-DD line. Follow with either `Rest Day`, `Rest Day: <reason>`, or workout-builder text lines. Use `Rest Day: <reason>` when you intentionally prescribe full rest so the backend can persist the reason. On every non-rest workout day, the first line after the date MUST be a short workout name (for example `Endurance` or `Sub-Threshold Durability`); never start a workout day directly with `-` steps. The backend falls back to `Planned workout` when a name is missing, but you must always provide a descriptive name. Block titles and descriptions are allowed on lines that do not start with `- ` and do not end with `x`. Step syntax: `- [Duration] [Target]`. Ramp syntax: `- [Duration] ramp [Start Target]-[End Target]`. Repeat headers must end with `x`, such as `Main Set 4x`. Supported durations: `30s`, `5m`, `45m`. Supported targets: `65%`, `95-105%`, `120-160W`. Example step: `- 45m 65%`. Example `plan` value: `2026-04-06\nEndurance\nWarmup\n- 15m ramp 100-270W\n2026-04-07\nRest Day: accumulated fatigue after race block\n2026-04-08\nRecovery Spin\n- 45m 55%`. Do not use cadence, zone targets, inline text cues, hour units, or distance units because the current backend parser does not accept them. For correction prompts, only output corrected dated sections for the invalid dates you are fixing inside the `plan` field.";
const TRAINING_PLAN_PLANNING_GUIDELINES_BASE: &str = "Planning guidelines: Follow a durability-first approach. Road cycling, especially masters racing, is stochastic; prioritize power repeatability and lactate clearance over pure steady-state aerobic work. Treat athlete age 45+, body-weight changes, and medications such as beta-blockers as fixed environmental constraints, not pathologies. Metric hierarchy: RPE over power over TSS/TSB over heart rate. If RPE stays low or moderate despite high fatigue metrics, trust recovery capacity and maintain load. Ignore heart rate for intensity pacing when beta-blockers are present. Never prescribe more than 2 consecutive Rest Day entries unless the athlete explicitly reports illness or injury. During build phases, TSB/Form may sit in the -15 to -25 range without forcing emergency rest. Prevent detraining by preferring Active Recovery or Z1 over total inactivity when extra recovery is needed. If the athlete reports fatigue or low freshness, first choose a short Z1 ride when availability allows a safe low-load session; prescribe Rest Day only when availability blocks even an easy ride or the context clearly supports full rest, and include a short concrete reason after `Rest Day:`. Plan beyond isolated days: shape the 14-day window as part of a coherent mesocycle with a clear phase progression, not a pile of disconnected sessions. Weekly load progression should be intentional. Read rc.pri for race priority: when pri is A or B, apply the high-frequency racing strategist A/B rules; when rc.pri is missing or ambiguous, default Category C. For Category C races, do not taper: treat the race like a high-intensity stochastic interval session, keep normal training load during race week, keep Tuesday and Wednesday interval sessions before a Sunday race when the context supports it, allow at most one light spinning or Rest Day on Friday or Saturday before the race, and schedule recovery or light endurance the day after the race before returning to structured intervals within 48 hours. When race time is materially earlier than normal training time, gradually shift key sessions toward the race start window to support circadian rhythm and heat adaptation.";
const TRAINING_PLAN_CONVERSATION_GUIDANCE: &str = "If earlier conversation messages are present, treat them as the exact conversation that led to this plan. Earlier assistant-role messages are your own earlier coach statements. If those earlier coach statements promised specific workouts, sequencing, or an easy/recovery/rest week structure, return a plan that stays consistent with those promises unless the packed training context clearly makes them unsafe or impossible. When you must override an earlier promise for safety, availability, or hard context constraints, stay as close as possible to the original intent and preserve any easy/recovery character of the block.";
const TRAINING_PLAN_FORWARD_LOAD_GUIDANCE: &str = "Forecast load sequentially before choosing each next day. Start from the current historical CTL, ATL, and TSB in the packed training context. Treat previously projected planned days (`pd`) as already planned/completed inputs when they exist, then simulate the effect of each newly planned workout before choosing the following day. Do not plan all 14 days from one static CTL/ATL/TSB snapshot. If the conversation or context says rest week, easy week, or recovery block, keep the forward simulation aligned with that low-load intent and avoid hard sessions unless they are truly necessary.";
const TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE: &str = "Weekly availability is mandatory and must be respected: only schedule workouts on weekdays marked available, keep unavailable days as Rest Day with a reason when full rest is intentional, and never exceed the configured max duration minutes for each available weekday.";
const TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE: &str = "Weekly availability is not configured in this context. Do not infer unavailable days or extra rest constraints from missing availability data. Plan a sensible 14-day cycling window from the training context alone, and avoid claiming that weekly availability is configured.";

fn with_planning_horizon(text: &str, window_day_count: usize) -> String {
    text.replace("14-day", &format!("{window_day_count}-day"))
        .replace("14 days", &format!("{window_day_count} days"))
}

pub fn training_plan_planning_guidelines(
    availability_configured: bool,
    window_day_count: usize,
) -> String {
    let availability_guidance = if availability_configured {
        TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE.to_string()
    } else {
        with_planning_horizon(
            TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE,
            window_day_count,
        )
    };
    format!(
        "{} {} {} {} {availability_guidance}",
        with_planning_horizon(TRAINING_PLAN_PLANNING_GUIDELINES_BASE, window_day_count),
        TRAINING_PLAN_CONVERSATION_GUIDANCE,
        with_planning_horizon(TRAINING_PLAN_FORWARD_LOAD_GUIDANCE, window_day_count),
        racing_strategist_plan_guidance(),
    )
}

pub fn training_plan_output_grammar() -> &'static str {
    TRAINING_PLAN_OUTPUT_GRAMMAR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_grammar_requires_workout_name_before_steps() {
        let grammar = training_plan_output_grammar();

        assert!(grammar.contains("first line after the date MUST be a short workout name"));
        assert!(grammar.contains("falls back to `Planned workout`"));
        assert!(grammar.contains("never start a workout day directly with `-` steps"));
    }

    #[test]
    fn planning_guidelines_include_racing_strategist_guidance() {
        let guidelines = training_plan_planning_guidelines(false, 14);
        assert!(guidelines.contains("Seiler 2010"));
        assert!(guidelines.contains("simulate_forward_load"));
        assert!(guidelines.contains("rc.pri"));
        assert!(guidelines.contains("durability-first"));
    }

    #[test]
    fn planning_guidelines_use_requested_horizon() {
        let meso = training_plan_planning_guidelines(false, 30);
        assert!(meso.contains("shape the 30-day window"));
        assert!(meso.contains("Do not plan all 30 days"));
        assert!(meso.contains("30-day cycling window"));

        let training_plan = training_plan_planning_guidelines(false, 14);
        assert!(training_plan.contains("shape the 14-day window"));
        assert!(training_plan.contains("Do not plan all 14 days"));
    }
}
