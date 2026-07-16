use super::anomaly::detect_anomalies;
use super::model::{AlignedInterval, CadenceRange, PlannedStep, WorkoutAnomaly};
use super::np::normalized_power;

/// Build one `AlignedInterval` from the planned step and its aligned actual slice.
pub fn build_interval(
    interval_index: usize,
    step: &PlannedStep,
    power: &[i32],
    cadence: &[i32],
) -> AlignedInterval {
    let anomalies = if step.step_type == super::model::StepType::Work {
        detect_anomalies(step, power, cadence)
    } else {
        Vec::new()
    };

    let avg_power = mean_i32(power);
    let normalized_power = normalized_power(power);
    let (avg_cadence, cadence_range) = active_cadence(cadence, &anomalies);

    AlignedInterval {
        interval_index,
        planned_step: step.clone(),
        actual_duration_seconds: power.len() as i32,
        avg_power,
        normalized_power,
        avg_cadence,
        cadence_range,
        anomalies,
    }
}

/// Average + min/max cadence over seconds NOT inside an anomaly window and NOT at 0 RPM.
fn active_cadence(cadence: &[i32], anomalies: &[WorkoutAnomaly]) -> (i32, CadenceRange) {
    let active: Vec<i32> = cadence
        .iter()
        .enumerate()
        .filter(|(i, _)| !inside_anomaly(*i, anomalies))
        .map(|(_, &c)| c)
        .filter(|&c| c > 0)
        .collect();
    if active.is_empty() {
        return (0, CadenceRange { min: 0, max: 0 });
    }
    let avg = mean_i32(&active);
    let min = *active.iter().min().unwrap_or(&0);
    let max = *active.iter().max().unwrap_or(&0);
    (avg, CadenceRange { min, max })
}

fn inside_anomaly(second: usize, anomalies: &[WorkoutAnomaly]) -> bool {
    anomalies.iter().any(|a| {
        let start = a.offset_seconds as usize;
        let end = start + a.duration_seconds as usize;
        (start..end).contains(&second)
    })
}

fn mean_i32(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    let sum: i64 = samples.iter().map(|&v| i64::from(v)).sum();
    (sum as f64 / samples.len() as f64).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workout_alignment::model::StepType;

    fn work(target: i32, dur: i32) -> PlannedStep {
        PlannedStep {
            name: "work".into(),
            step_type: StepType::Work,
            target_power_min: target,
            target_power_max: target,
            planned_duration_seconds: dur,
        }
    }

    #[test]
    fn steady_block_metrics() {
        let step = work(250, 60);
        let power = vec![250; 60];
        let cadence = vec![85; 60];
        let interval = build_interval(0, &step, &power, &cadence);
        assert_eq!(interval.avg_power, 250);
        assert_eq!(interval.avg_cadence, 85);
        assert_eq!(interval.cadence_range, CadenceRange { min: 85, max: 85 });
        assert!(interval.anomalies.is_empty());
    }

    #[test]
    fn anomaly_window_excluded_from_cadence_average() {
        // 60s @ 85 rpm, with a 10s coasting stop in the middle (0 rpm).
        let step = work(250, 60);
        let mut power = vec![250; 60];
        let mut cadence = vec![85; 60];
        for i in 25..35 {
            power[i] = 0;
            cadence[i] = 0;
        }
        let interval = build_interval(0, &step, &power, &cadence);
        assert_eq!(interval.anomalies.len(), 1);
        // Active cadence only: all 85 → average stays 85 despite the 0s block.
        assert_eq!(interval.avg_cadence, 85);
    }

    #[test]
    fn recovery_step_has_no_anomalies_even_on_zeros() {
        let step = PlannedStep {
            name: "recovery".into(),
            step_type: StepType::Recovery,
            target_power_min: 100,
            target_power_max: 100,
            planned_duration_seconds: 60,
        };
        let power = vec![0; 60];
        let cadence = vec![0; 60];
        let interval = build_interval(0, &step, &power, &cadence);
        assert!(interval.anomalies.is_empty());
    }
}
