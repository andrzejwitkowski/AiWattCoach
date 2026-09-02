use super::model::{work_power_drop_threshold, PlannedStep, StepType};

/// Per-second aligned slices `[start, end)` for each planned step, in order.
/// `end` of step `s` constrains `start` of step `s+1` (monotonic).
pub type StepSlices = Vec<(usize, usize)>;

// ponytail: monotonic segmented DP, not full Sakoe-Chiba DTW. The planned axis is
// piecewise-constant steps, so optimal per-step boundary search is equivalent to
// banded DTW for this use case. Upgrade to full DTW if real rides show pathological
// non-monotonic warping (out-of-order intervals).
//
// Cost = 0.5 * power_similarity + 0.5 * duration_similarity.
// Work steps score power on non-coasting seconds so a mid-block turn does not make a
// shorter clean slice cheaper than the full planned span.

const POWER_WEIGHT: f64 = 0.5;
const DURATION_WEIGHT: f64 = 0.5;
const MIN_BAND_SECONDS: i32 = 120;

/// Align actual `power` samples to the planned step sequence.
///
/// Returns one `(start, end)` half-open slice per planned step. Steps may map to
/// empty slices if the actual stream is too short; warmup/cooldown seconds beyond
/// the planned span are absorbed into the first/last step with light penalty.
pub fn align(planned: &[PlannedStep], power: &[i32]) -> StepSlices {
    if planned.is_empty() || power.is_empty() {
        return planned.iter().map(|_| (0usize, 0usize)).collect();
    }
    if planned.len() == 1 {
        return vec![(0, power.len())];
    }

    let n = power.len();
    let s = planned.len();
    let power_prefix = prefix_sums(power);

    let cumulative_planned = cumulative_durations(planned);
    let total_planned = cumulative_planned[s];

    // dp[j] = min cost to align first `step+1` steps ending at actual index `j`.
    // Keep back-pointers for EVERY layer so the full path can be reconstructed.
    let mut layers: Vec<(Vec<f64>, Vec<Option<usize>>)> = Vec::with_capacity(s);

    // Step 0: leading warmup absorbed, no prior boundary.
    let mut dp = vec![f64::INFINITY; n + 1];
    let mut back: Vec<Option<usize>> = vec![None; n + 1];
    for j in 1..=n {
        dp[j] = step_cost(&planned[0], power, &power_prefix, 0, j);
        back[j] = Some(0);
    }
    layers.push((dp, back));

    for step in 1..s {
        let (prev_dp, _) = &layers[step - 1];
        let mut cur_dp = vec![f64::INFINITY; n + 1];
        let mut cur_back: Vec<Option<usize>> = vec![None; n + 1];
        let band = band_seconds(planned[step].planned_duration_seconds);

        for j in step + 1..=n {
            // Prior boundary i in [step-1 .. j-1]; step duration band constrains i.
            let target_i = j.saturating_sub(planned[step].planned_duration_seconds as usize);
            let i_lo = target_i.saturating_sub(band).max(step - 1);
            let i_hi = (target_i + band).min(j - 1);
            let best = (i_lo..=i_hi)
                .filter_map(|i| {
                    let prior = prev_dp[i];
                    if prior.is_finite() && i < j {
                        Some((
                            prior + step_cost(&planned[step], power, &power_prefix, i, j),
                            i,
                        ))
                    } else {
                        None
                    }
                })
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            if let Some((cost, i)) = best {
                cur_dp[j] = cost;
                cur_back[j] = Some(i);
            }
        }
        layers.push((cur_dp, cur_back));
    }

    // Best endpoint absorbs trailing cooldown into the last step.
    let (final_dp, _) = &layers[s - 1];
    let total_band = band_seconds(i32::try_from(total_planned).unwrap_or(i32::MAX));
    let end_lo = n.saturating_sub(total_band);
    let end = (end_lo..=n)
        .filter_map(|j| final_dp[j].is_finite().then_some((final_dp[j], j)))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, j)| j)
        .unwrap_or(n);

    backtrack_layers(&layers, end, s)
}

fn band_seconds(duration_seconds: i32) -> usize {
    // ponytail: single tunable — max(120s, 30% of planned duration).
    ((duration_seconds as f64 * 0.30).round() as i32).max(MIN_BAND_SECONDS) as usize
}

fn cumulative_durations(planned: &[PlannedStep]) -> Vec<usize> {
    let mut acc = 0usize;
    let mut out = Vec::with_capacity(planned.len() + 1);
    out.push(0);
    for step in planned {
        acc += step.planned_duration_seconds.max(0) as usize;
        out.push(acc);
    }
    out
}

fn prefix_sums(power: &[i32]) -> Vec<i64> {
    let mut prefix = vec![0i64; power.len() + 1];
    for (index, value) in power.iter().enumerate() {
        prefix[index + 1] = prefix[index] + i64::from(*value);
    }
    prefix
}

fn slice_mean(prefix: &[i64], start: usize, end: usize) -> f64 {
    if start >= end {
        return 0.0;
    }
    let sum = prefix[end] - prefix[start];
    sum as f64 / (end - start) as f64
}

/// Mean excluding coasting seconds (`p < work_power_drop_threshold`). Falls back to
/// full-slice mean when every second is coasting.
fn slice_mean_excluding_coasting(
    power: &[i32],
    prefix: &[i64],
    start: usize,
    end: usize,
    coast_threshold: i32,
) -> f64 {
    if start >= end {
        return 0.0;
    }
    let (sum, count) = power[start..end].iter().fold((0i64, 0i64), |(s, n), &p| {
        if p >= coast_threshold {
            (s + i64::from(p), n + 1)
        } else {
            (s, n)
        }
    });
    if count == 0 {
        return slice_mean(prefix, start, end);
    }
    sum as f64 / count as f64
}

fn step_cost(
    step: &PlannedStep,
    power: &[i32],
    power_prefix: &[i64],
    start: usize,
    end: usize,
) -> f64 {
    if start >= end {
        return 1.0;
    }
    let expected_watts = expected_power(step);
    let block_mean = if step.step_type == StepType::Work {
        slice_mean_excluding_coasting(
            power,
            power_prefix,
            start,
            end,
            work_power_drop_threshold(step.target_power_min),
        )
    } else {
        slice_mean(power_prefix, start, end)
    };
    let power_sim = similarity(block_mean, expected_watts);
    let dur_sim = similarity((end - start) as f64, step.planned_duration_seconds as f64);
    1.0 - (POWER_WEIGHT * power_sim + DURATION_WEIGHT * dur_sim)
}

fn expected_power(step: &PlannedStep) -> f64 {
    (step.target_power_min + step.target_power_max) as f64 / 2.0
}

fn similarity(actual: f64, expected: f64) -> f64 {
    if expected <= 0.0 {
        return 0.0;
    }
    (1.0 - ((actual - expected).abs() / expected)).clamp(0.0, 1.0)
}

fn backtrack_layers(
    layers: &[(Vec<f64>, Vec<Option<usize>>)],
    final_end: usize,
    steps: usize,
) -> StepSlices {
    let mut boundaries = Vec::with_capacity(steps + 1);
    let mut cursor = final_end;
    boundaries.push(cursor);
    for step in (1..steps).rev() {
        cursor = layers[step].1.get(cursor).copied().flatten().unwrap_or(0);
        boundaries.push(cursor);
    }
    boundaries.push(0);
    boundaries.reverse();

    boundaries
        .windows(2)
        .map(|w| (w[0].min(w[1]), w[0].max(w[1])))
        .take(steps)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workout_alignment::model::StepType;

    fn step(name: &str, watts: i32, dur: i32) -> PlannedStep {
        PlannedStep {
            name: name.into(),
            step_type: StepType::Work,
            target_power_min: watts,
            target_power_max: watts,
            planned_duration_seconds: dur,
        }
    }

    #[test]
    fn empty_inputs_return_empty_slices() {
        let slices = align(&[], &[]);
        assert!(slices.is_empty());
    }

    #[test]
    fn single_step_covers_full_stream() {
        let planned = vec![step("block", 250, 60)];
        let power: Vec<i32> = vec![250; 60];
        let slices = align(&planned, &power);
        assert_eq!(slices, vec![(0, 60)]);
    }

    #[test]
    fn three_step_ride_aligns_to_boundaries() {
        // 60s @ 100W, 60s @ 300W, 60s @ 100W — clear on/off/on boundaries.
        let planned = vec![
            step("warmup", 100, 60),
            step("work", 300, 60),
            step("cooldown", 100, 60),
        ];
        let mut power = vec![100; 60];
        power.extend(vec![300; 60]);
        power.extend(vec![100; 60]);
        let slices = align(&planned, &power);
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].1 - slices[0].0, 60, "warmup span");
        assert_eq!(slices[1].1 - slices[1].0, 60, "work span");
        assert_eq!(slices[2].1 - slices[2].0, 60, "cooldown span");
        let work_block = &power[slices[1].0..slices[1].1];
        let work_mean: f64 = work_block.iter().map(|&v| v as f64).sum::<f64>() / 60.0;
        assert!(
            (work_mean - 300.0).abs() < 5.0,
            "work mean ~300W, got {work_mean}"
        );
    }

    #[test]
    fn longer_flat_actual_aligns_one_slice_per_step() {
        // Plan is [100W x 30s, 200W x 30s]; actual is 90s of flat 100W.
        // The aligner must still emit one slice per planned step with contiguous,
        // ordered, non-overlapping boundaries. It is not required to consume every
        // actual second when trailing data doesn't match any step target.
        let planned = vec![step("a", 100, 30), step("b", 200, 30)];
        let power: Vec<i32> = vec![100; 90];
        let slices = align(&planned, &power);
        assert_eq!(slices.len(), 2);
        // Boundaries are contiguous and ordered: step0 ends where step1 begins.
        assert_eq!(slices[0].1, slices[1].0);
        assert!(slices[0].0 <= slices[0].1);
        assert!(slices[1].0 <= slices[1].1);
        assert!(slices[1].1 <= 90);
    }

    #[test]
    fn band_seconds_uses_thirty_percent_floor_120() {
        assert_eq!(band_seconds(60), 120);
        assert_eq!(band_seconds(600), 180);
        assert_eq!(band_seconds(1000), 300);
    }

    #[test]
    fn similarity_clamps_to_zero_on_large_deviation() {
        assert!((similarity(300.0, 250.0) - 0.8).abs() < 1e-9);
        assert_eq!(similarity(0.0, 250.0), 0.0);
        assert_eq!(similarity(250.0, 0.0), 0.0);
    }

    #[test]
    fn masked_mean_ignores_coasting_seconds() {
        let mut power = vec![300; 50];
        power.extend(vec![0; 20]);
        power.extend(vec![300; 30]);
        let prefix = prefix_sums(&power);
        let mean = slice_mean_excluding_coasting(&power, &prefix, 0, 100, 150);
        assert!((mean - 300.0).abs() < 1e-9);
    }

    #[test]
    fn work_cost_prefers_full_span_despite_mid_coast() {
        let work = step("work", 300, 100);
        let mut power = vec![300; 50];
        power.extend(vec![0; 20]);
        power.extend(vec![300; 30]);
        let prefix = prefix_sums(&power);
        let full = step_cost(&work, &power, &prefix, 0, 100);
        let cut = step_cost(&work, &power, &prefix, 0, 50);
        assert!(
            full < cut,
            "full span with masked coast should beat short clean cut: full={full} cut={cut}"
        );
    }

    #[test]
    fn mid_block_coast_then_resume_keeps_work_near_planned_duration() {
        let planned = vec![
            step("warmup", 100, 30),
            step("work", 300, 90),
            step("cool", 100, 30),
        ];
        let mut power = vec![100; 30];
        power.extend(vec![300; 40]);
        power.extend(vec![0; 25]);
        power.extend(vec![300; 25]);
        power.extend(vec![100; 30]);
        let slices = align(&planned, &power);
        let work_len = slices[1].1 - slices[1].0;
        assert!(
            work_len >= 80,
            "work should not end at coast; got {work_len}s (planned 90)"
        );
        assert_eq!(slices[0].1, slices[1].0);
        assert_eq!(slices[1].1, slices[2].0);
    }

    #[test]
    fn sst_second_interval_not_clipped_at_mid_turn() {
        let planned = vec![
            step("wu", 200, 60),
            step("w1", 300, 480),
            step("r1", 220, 180),
            step("w2", 300, 480),
            step("r2", 220, 180),
            step("cd", 180, 60),
        ];
        let mut power = vec![200; 60];
        power.extend(vec![300; 480]);
        power.extend(vec![220; 180]);
        power.extend(vec![300; 350]);
        power.extend(vec![0; 100]);
        power.extend(vec![300; 30]);
        power.extend(vec![220; 180]);
        power.extend(vec![180; 60]);
        let slices = align(&planned, &power);
        let w2 = slices[3].1 - slices[3].0;
        assert!(
            w2 >= 450,
            "second SST must keep post-turn watts; got {w2}s (planned 480)"
        );
    }
}
