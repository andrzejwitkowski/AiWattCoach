pub mod align;
pub mod anomaly;
pub mod metrics;
pub mod model;
pub mod np;

pub use align::StepSlices;
pub use model::{
    AlignedInterval, AnomalyType, CadenceRange, PlannedStep, StepType, WorkoutAnomaly,
};

/// Inputs for plan-vs-actual alignment derived from a planned-workout block.
#[derive(Clone, Debug)]
pub struct PlannedBlockInput {
    pub name: Option<String>,
    pub duration_seconds: i32,
    pub min_percent_ftp: Option<f64>,
    pub max_percent_ftp: Option<f64>,
    pub min_target_watts: Option<i32>,
    pub max_target_watts: Option<i32>,
}

pub fn align_workout_from_doc(
    workout_doc: Option<&str>,
    ftp_watts: Option<i32>,
    power: &[i32],
    cadence: &[i32],
) -> Option<Vec<AlignedInterval>> {
    let parsed = crate::domain::intervals::parse_workout_doc(workout_doc, ftp_watts);
    let blocks = parsed
        .segments
        .iter()
        .map(|segment| PlannedBlockInput {
            name: Some(segment.label.clone()),
            duration_seconds: segment.duration_seconds,
            min_percent_ftp: segment.min_target_percent_ftp,
            max_percent_ftp: segment.max_target_percent_ftp,
            min_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .min_target_percent_ftp
                    .map(|pct| (f64::from(ftp) * pct / 100.0).round() as i32)
            }),
            max_target_watts: ftp_watts.and_then(|ftp| {
                segment
                    .max_target_percent_ftp
                    .map(|pct| (f64::from(ftp) * pct / 100.0).round() as i32)
            }),
        })
        .collect::<Vec<_>>();
    align_workout_from_blocks(&blocks, ftp_watts, power, cadence)
}

pub fn align_workout_from_blocks(
    blocks: &[PlannedBlockInput],
    ftp_watts: Option<i32>,
    power: &[i32],
    cadence: &[i32],
) -> Option<Vec<AlignedInterval>> {
    if blocks.is_empty() || power.is_empty() {
        return None;
    }
    let planned: Vec<PlannedStep> = blocks
        .iter()
        .enumerate()
        .map(|(i, block)| planned_step_from_block(i, block, ftp_watts))
        .collect();
    Some(align_workout(&planned, power, cadence))
}

fn planned_step_from_block(
    index: usize,
    block: &PlannedBlockInput,
    ftp: Option<i32>,
) -> PlannedStep {
    let step_type = step_type_from_percent(block.min_percent_ftp, block.max_percent_ftp);
    let (target_power_min, target_power_max) = resolve_watts(block, ftp);
    PlannedStep {
        name: block
            .name
            .clone()
            .unwrap_or_else(|| format!("step {}", index + 1)),
        step_type,
        target_power_min,
        target_power_max,
        planned_duration_seconds: block.duration_seconds,
    }
}

fn step_type_from_percent(min_percent_ftp: Option<f64>, max_percent_ftp: Option<f64>) -> StepType {
    let upper_bound = max_percent_ftp.or(min_percent_ftp);
    match upper_bound {
        Some(pct) if pct <= model::RECOVERY_PERCENT_FTP => StepType::Recovery,
        _ => StepType::Work,
    }
}

fn resolve_watts(block: &PlannedBlockInput, ftp: Option<i32>) -> (i32, i32) {
    if let (Some(min), Some(max)) = (block.min_target_watts, block.max_target_watts) {
        return (min, max);
    }
    match (ftp, block.min_percent_ftp, block.max_percent_ftp) {
        (Some(ftp), Some(min_pct), Some(max_pct)) => {
            let to_w = |pct: f64| (f64::from(ftp) * pct / 100.0).round() as i32;
            (to_w(min_pct), to_w(max_pct))
        }
        _ => (0, 0),
    }
}

/// Align per-second streams onto planned steps; one `AlignedInterval` per step.
pub fn align_workout(
    planned: &[PlannedStep],
    power: &[i32],
    cadence: &[i32],
) -> Vec<AlignedInterval> {
    let slices = align::align(planned, power);
    planned
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let (start, end) = slices.get(i).copied().unwrap_or((0, 0));
            let step_power = power.get(start..end).unwrap_or(&[]);
            let step_cadence = cadence.get(start..end).unwrap_or(&[]);
            metrics::build_interval(i, step, step_power, step_cadence)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(name: &str, target: i32, dur: i32) -> PlannedStep {
        PlannedStep {
            name: name.into(),
            step_type: StepType::Work,
            target_power_min: target,
            target_power_max: target,
            planned_duration_seconds: dur,
        }
    }

    fn recovery(name: &str, dur: i32) -> PlannedStep {
        PlannedStep {
            name: name.into(),
            step_type: StepType::Recovery,
            target_power_min: 100,
            target_power_max: 100,
            planned_duration_seconds: dur,
        }
    }

    #[test]
    fn end_to_end_classic_interval_workout() {
        // 30s warmup @ 100, 90s work @ 300 (with a 10s coasting stop), 30s cooldown @ 100.
        let planned = vec![
            work("warmup", 100, 30),
            work("3m@300", 300, 90),
            recovery("recovery", 30),
        ];
        let mut power = vec![100; 30];
        power.extend(vec![300; 40]);
        power.extend(vec![0; 10]); // coasting stop mid-work
        power.extend(vec![300; 40]);
        power.extend(vec![100; 30]);

        let mut cadence = vec![85; 30];
        cadence.extend(vec![90; 40]);
        cadence.extend(vec![0; 10]); // stopped pedaling
        cadence.extend(vec![90; 40]);
        cadence.extend(vec![80; 30]);

        let intervals = align_workout(&planned, &power, &cadence);
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].planned_step.name, "warmup");
        // Work interval sees exactly one coasting-stop anomaly.
        let work = &intervals[1];
        assert_eq!(work.anomalies.len(), 1);
        assert_eq!(work.anomalies[0].anomaly_type, AnomalyType::CoastingStop);
        assert_eq!(work.anomalies[0].duration_seconds, 10);
        // Active cadence for work excludes the 10s of 0 rpm.
        assert!(
            work.avg_cadence >= 89,
            "work avg cadence {}",
            work.avg_cadence
        );
        // Recovery never scanned.
        assert!(intervals[2].anomalies.is_empty());
    }

    #[test]
    fn empty_actual_returns_zeroed_intervals() {
        let planned = vec![work("x", 250, 60)];
        let intervals = align_workout(&planned, &[], &[]);
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].avg_power, 0);
    }

    #[test]
    fn no_planned_steps_returns_empty() {
        assert!(align_workout(&[], &[250; 60], &[85; 60]).is_empty());
    }

    #[test]
    fn ramp_with_high_upper_bound_is_work_not_recovery() {
        let block = PlannedBlockInput {
            name: Some("ramp".into()),
            duration_seconds: 300,
            min_percent_ftp: Some(50.0),
            max_percent_ftp: Some(100.0),
            min_target_watts: Some(125),
            max_target_watts: Some(250),
        };
        let step = planned_step_from_block(0, &block, Some(250));
        assert_eq!(step.step_type, StepType::Work);
    }
}
