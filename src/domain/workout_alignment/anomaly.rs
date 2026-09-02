use super::model::{AnomalyType, PlannedStep, StepType, WorkoutAnomaly};

// Coasting threshold shared with `workout_streams/segments.rs:POWER_COASTING_THRESHOLD_WATTS`.
const COASTING_THRESHOLD_WATTS: i32 = 10;
// Trigger gap below which anomaly seconds merge into one event.
const MERGE_GAP_SECONDS: usize = 2;

/// Detect unplanned drops inside a single aligned `work` step.
/// Recovery steps are never scanned (callers gate on `step_type`).
///
/// Trigger second: power < 50% of target_power_min, OR power == 0, OR cadence == 0.
/// Consecutive triggers (gaps <= MERGE_GAP_SECONDS) collapse into one `WorkoutAnomaly`.
pub fn detect_anomalies(step: &PlannedStep, power: &[i32], cadence: &[i32]) -> Vec<WorkoutAnomaly> {
    if step.step_type != StepType::Work || power.is_empty() {
        return Vec::new();
    }
    let threshold = super::model::work_power_drop_threshold(step.target_power_min);
    let has_cadence = !cadence.is_empty();
    let trigger_indices: Vec<usize> = power
        .iter()
        .enumerate()
        .filter_map(|(i, &p)| {
            let cadence_zero = has_cadence && cadence.get(i).copied() == Some(0);
            (p < threshold || p == 0 || cadence_zero).then_some(i)
        })
        .collect();

    let mut anomalies = Vec::new();
    for group in group_consecutive(&trigger_indices, MERGE_GAP_SECONDS) {
        if group.is_empty() {
            continue;
        }
        let offset = group[0] as i32;
        let duration = (group[group.len() - 1] - group[0] + 1) as i32;
        let avg_power = mean_at(power, &group);
        let avg_cadence = mean_at(cadence, &group);
        anomalies.push(WorkoutAnomaly {
            offset_seconds: offset,
            duration_seconds: duration,
            avg_power,
            avg_cadence,
            anomaly_type: classify(avg_power, avg_cadence, threshold),
        });
    }
    anomalies
}

// ponytail: coasting stop vs turn split via the existing 10W threshold instead of
// ramp-rate analysis. Tune against real rides if miscategorised.
fn classify(avg_power: i32, avg_cadence: i32, power_drop_threshold: i32) -> AnomalyType {
    if avg_cadence == 0 {
        if avg_power < COASTING_THRESHOLD_WATTS {
            AnomalyType::CoastingStop
        } else {
            AnomalyType::CoastingTurn
        }
    } else if avg_power < power_drop_threshold {
        AnomalyType::SignificantPowerDrop
    } else {
        // Cadence-zero brief blip with otherwise on-target power: treat as turn.
        AnomalyType::CoastingTurn
    }
}

fn mean_at(values: &[i32], indices: &[usize]) -> i32 {
    if indices.is_empty() {
        return 0;
    }
    let sum: i64 = indices
        .iter()
        .map(|&i| i64::from(values.get(i).copied().unwrap_or(0)))
        .sum();
    (sum as f64 / indices.len() as f64).round() as i32
}

fn group_consecutive(indices: &[usize], max_gap: usize) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for &idx in indices {
        match current.last() {
            Some(&prev) if idx <= prev + 1 + max_gap => current.push(idx),
            Some(_) => {
                groups.push(std::mem::take(&mut current));
                current.push(idx);
            }
            None => current.push(idx),
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(target_min: i32) -> PlannedStep {
        PlannedStep {
            name: "work".into(),
            step_type: StepType::Work,
            target_power_min: target_min,
            target_power_max: target_min,
            planned_duration_seconds: 120,
        }
    }

    #[test]
    fn recovery_step_never_scanned() {
        let step = PlannedStep {
            name: "recovery".into(),
            step_type: StepType::Recovery,
            target_power_min: 100,
            target_power_max: 100,
            planned_duration_seconds: 60,
        };
        let power = vec![0; 60];
        let cadence = vec![0; 60];
        assert!(detect_anomalies(&step, &power, &cadence).is_empty());
    }

    #[test]
    fn steady_on_target_emits_no_anomaly() {
        let step = work(250);
        let power = vec![250; 60];
        let cadence = vec![85; 60];
        assert!(detect_anomalies(&step, &power, &cadence).is_empty());
    }

    #[test]
    fn red_light_stop_classifies_as_coasting_stop() {
        let step = work(250);
        let mut power = vec![250; 60];
        let mut cadence = vec![85; 60];
        // 12s stop mid-block: power 0, cadence 0.
        for i in 25..37 {
            power[i] = 0;
            cadence[i] = 0;
        }
        let anomalies = detect_anomalies(&step, &power, &cadence);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::CoastingStop);
        assert_eq!(anomalies[0].offset_seconds, 25);
        assert_eq!(anomalies[0].duration_seconds, 12);
        assert_eq!(anomalies[0].avg_power, 0);
    }

    #[test]
    fn corner_coast_classifies_as_coasting_turn() {
        let step = work(250);
        let mut power = vec![250; 60];
        let mut cadence = vec![85; 60];
        // Stop pedaling but still rolling: low but nonzero power, cadence 0.
        for i in 25..32 {
            power[i] = 50; // > COASTING_THRESHOLD_WATTS
            cadence[i] = 0;
        }
        let anomalies = detect_anomalies(&step, &power, &cadence);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::CoastingTurn);
        assert_eq!(anomalies[0].duration_seconds, 7);
    }

    #[test]
    fn fatigue_drop_classifies_as_significant_power_drop() {
        let step = work(250);
        let mut power = vec![250; 60];
        let cadence = vec![85; 60]; // cadence stays active
        for value in &mut power[30..40] {
            *value = 80; // well below 50% target (125)
        }
        let anomalies = detect_anomalies(&step, &power, &cadence);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::SignificantPowerDrop);
        assert_eq!(anomalies[0].offset_seconds, 30);
    }

    #[test]
    fn nearby_anomaly_groups_merge() {
        let step = work(250);
        let mut power = vec![250; 60];
        let mut cadence = vec![85; 60];
        // Two drops 2s apart: gap <= MERGE_GAP_SECONDS → single anomaly.
        power[20] = 0;
        cadence[20] = 0;
        power[23] = 0;
        cadence[23] = 0;
        let anomalies = detect_anomalies(&step, &power, &cadence);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].offset_seconds, 20);
        assert_eq!(anomalies[0].duration_seconds, 4);
    }

    #[test]
    fn missing_cadence_stream_does_not_trigger_cadence_zero() {
        let step = work(250);
        let power = vec![250; 60];
        let cadence: Vec<i32> = vec![];
        assert!(detect_anomalies(&step, &power, &cadence).is_empty());
    }

    #[test]
    fn distant_anomalies_stay_separate() {
        let step = work(250);
        let mut power = vec![250; 60];
        let mut cadence = vec![85; 60];
        power[10] = 0;
        cadence[10] = 0;
        power[40] = 0;
        cadence[40] = 0;
        assert_eq!(detect_anomalies(&step, &power, &cadence).len(), 2);
    }
}
