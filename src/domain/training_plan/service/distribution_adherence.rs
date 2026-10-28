use crate::domain::{
    intervals::{zone_for_percent, PlannedWorkoutLine, PlannedWorkoutTarget},
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
    for step in days
        .iter()
        .filter_map(|day| day.workout.as_ref())
        .flat_map(|workout| workout.lines.iter())
        .filter_map(PlannedWorkoutLine::step)
    {
        let seconds = i64::from(step.duration_seconds);
        match step.target {
            PlannedWorkoutTarget::PercentFtp { min, max } => {
                match zone_for_percent((min + max) / 2.0) {
                    1 | 2 => zones.low += seconds,
                    3 => zones.middle += seconds,
                    _ => zones.high += seconds,
                }
            }
            PlannedWorkoutTarget::WattsRange { .. } => zones.unclassified += seconds,
        }
    }
    zones
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
}
