use crate::domain::{
    intervals::{
        zone_for_percent, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
        PlannedWorkoutTarget,
    },
    settings::IntensityDistribution,
    training_plan::TrainingPlanDay,
};

/// Measurement basis: planned step seconds, classified by %FTP into a 3-zone model
/// (low = Z1-Z2, middle = Z3, high = Z4+ per the canonical `zone_for_percent` table).
/// Steps targeted in watts are not classified and are reported as unclassified.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct ZoneSeconds {
    pub low: i64,
    pub middle: i64,
    pub high: i64,
    pub unclassified: i64,
}

fn plan_zone_seconds(days: &[TrainingPlanDay]) -> ZoneSeconds {
    let mut zones = ZoneSeconds::default();
    for workout in days.iter().filter_map(|day| day.workout.as_ref()) {
        // A repeat header applies to the steps that follow it until a blank or text line.
        let mut repeat_count = 1_i64;
        for line in &workout.lines {
            match line {
                PlannedWorkoutLine::Repeat(repeat) => repeat_count = repeat.count as i64,
                PlannedWorkoutLine::BlankLine | PlannedWorkoutLine::Text(_) => repeat_count = 1,
                PlannedWorkoutLine::Step(step) => {
                    add_step_seconds(&mut zones, step, repeat_count);
                }
            }
        }
    }
    zones
}

const RAMP_SLICES: i64 = 100;

fn add_step_seconds(zones: &mut ZoneSeconds, step: &PlannedWorkoutStep, repeat_count: i64) {
    let seconds = i64::from(step.duration_seconds) * repeat_count;
    let PlannedWorkoutTarget::PercentFtp { min, max } = step.target else {
        zones.unclassified += seconds;
        return;
    };
    if step.kind == PlannedWorkoutStepKind::Ramp {
        // Ramp time is spread over the zones it crosses, not assigned to its midpoint.
        for slice in 0..RAMP_SLICES {
            let percent = min + (max - min) * (slice as f64 + 0.5) / RAMP_SLICES as f64;
            add_zone_seconds(zones, percent, seconds / RAMP_SLICES);
        }
    } else {
        add_zone_seconds(zones, (min + max) / 2.0, seconds);
    }
}

fn add_zone_seconds(zones: &mut ZoneSeconds, percent: f64, seconds: i64) {
    match zone_for_percent(percent) {
        1 | 2 => zones.low += seconds,
        3 => zones.middle += seconds,
        _ => zones.high += seconds,
    }
}

/// One evidence line for the quality evaluator. Compares time-in-zone ordering with the selected
/// model; it never reports compliance when the window cannot support the judgement.
pub fn distribution_adherence_line(
    days: &[TrainingPlanDay],
    distribution: IntensityDistribution,
) -> String {
    let zones = plan_zone_seconds(days);
    let counts = format!(
        "low={}m middle={}m high={}m unclassified={}m (basis: planned step time, %FTP 3-zone: Z1-2/Z3/Z4+)",
        zones.low / 60,
        zones.middle / 60,
        zones.high / 60,
        zones.unclassified / 60
    );
    let name = distribution.as_str();
    if zones.low == 0 || zones.middle + zones.high == 0 {
        return format!("distribution_adherence[{name}]: insufficient data to assess; {counts}");
    }
    let adheres = match distribution {
        IntensityDistribution::Polarized => zones.low > zones.high && zones.high > zones.middle,
        IntensityDistribution::Pyramidal => zones.low > zones.middle && zones.middle > zones.high,
        IntensityDistribution::Threshold => zones.middle >= zones.high && zones.middle > 0,
    };
    let verdict = if adheres {
        "consistent with"
    } else {
        "NOT consistent with"
    };
    format!("distribution_adherence[{name}]: time-in-zone {verdict} selected model; {counts}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intervals::parse_planned_workout_days;

    fn days(block: &str) -> Vec<TrainingPlanDay> {
        parse_planned_workout_days(block)
            .expect("parse")
            .days
            .into_iter()
            .map(|day| {
                let date = day.date.clone();
                let rest_day = day.is_rest_day();
                let rest_day_reason = day.rest_day_reason().map(ToString::to_string);
                TrainingPlanDay {
                    date,
                    rest_day,
                    rest_day_reason,
                    workout: day.into_workout(),
                }
            })
            .collect()
    }

    #[test]
    fn polarized_needs_high_time_above_middle_time() {
        let polarized =
            days("2026-10-01\nEndurance\n- 120m 65%\n2026-10-02\nVO2\n- 12m 110%\n- 10m 80%\n");
        let line = distribution_adherence_line(&polarized, IntensityDistribution::Polarized);
        assert!(line.contains("time-in-zone consistent with"), "{line}");
        let line = distribution_adherence_line(&polarized, IntensityDistribution::Pyramidal);
        assert!(line.contains("NOT consistent"), "{line}");
    }

    #[test]
    fn endurance_only_window_is_insufficient_not_compliant() {
        let easy = days("2026-10-01\nEndurance\n- 90m 65%\n");
        let line = distribution_adherence_line(&easy, IntensityDistribution::Polarized);
        assert!(line.contains("insufficient data"), "{line}");
    }

    #[test]
    fn repeat_blocks_count_every_execution() {
        // 4 x 5m @110% = 20m high; 30m @65% low. Without repeat expansion high (5m) would look tiny.
        let plan = days("2026-10-01\nVO2\n- 30m 65%\n\nMain Set 4x\n- 5m 110%\n");
        let zones = plan_zone_seconds(&plan);
        assert_eq!((zones.low, zones.high), (1800, 1200));
    }

    #[test]
    fn ramp_time_is_split_across_crossed_zones() {
        let plan = days("2026-10-01\nRamp\n- 20m ramp 50-120%\n");
        let zones = plan_zone_seconds(&plan);
        assert!(
            zones.low > 0 && zones.middle > 0 && zones.high > 0,
            "{zones:?}"
        );
        assert_eq!(zones.low + zones.middle + zones.high, 1200);
    }
}
