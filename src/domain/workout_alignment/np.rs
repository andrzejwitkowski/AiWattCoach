/// Canonical Coggan Normalized Power: 30s rolling-mean → cube → mean → 4th root.
/// Mirrors the `powi(4)`/`powf(0.25)` idiom from `intervals/workout/parser.rs`.
pub fn normalized_power(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    // ponytail: NP is physiologically undefined under 30s; fall back to mean.
    if samples.len() < ROLLING_WINDOW_SECONDS {
        return mean_i32(samples);
    }

    let rolling = rolling_means(samples, ROLLING_WINDOW_SECONDS);
    let mean_of_cubes =
        rolling.iter().map(|&m| (m as f64).powi(4)).sum::<f64>() / rolling.len() as f64;

    (mean_of_cubes.powf(0.25)).round() as i32
}

const ROLLING_WINDOW_SECONDS: usize = 30;

fn mean_i32(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    let sum: i64 = samples.iter().map(|&v| i64::from(v)).sum();
    (sum as f64 / samples.len() as f64).round() as i32
}

/// O(n) rolling mean via prefix sums. Returns `n - window + 1` values.
fn rolling_means(samples: &[i32], window: usize) -> Vec<i64> {
    if samples.len() < window || window == 0 {
        return Vec::new();
    }
    let mut prefix = vec![0i64; samples.len() + 1];
    for (i, &v) in samples.iter().enumerate() {
        prefix[i + 1] = prefix[i] + i64::from(v);
    }
    let denom = window as f64;
    (0..samples.len() + 1 - window)
        .map(|i| {
            let sum = prefix[i + window] - prefix[i];
            (sum as f64 / denom).round() as i64
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_samples_return_zero() {
        assert_eq!(normalized_power(&[]), 0);
    }

    #[test]
    fn short_block_falls_back_to_mean() {
        assert_eq!(normalized_power(&[100, 200, 300]), 200);
    }

    #[test]
    fn steady_power_equals_mean() {
        // 60s of constant 250W: NP == 250.
        let samples = vec![250; 60];
        assert_eq!(normalized_power(&samples), 250);
    }

    #[test]
    fn variable_power_exceeds_mean() {
        // First 30s @ 100W, next 30s @ 300W. Mean=200, but NP > 200 because
        // rolling windows spanning the transition vary window-to-window.
        let mut samples = vec![100; 30];
        samples.extend(vec![300; 30]);
        let np = normalized_power(&samples);
        let mean = 200;
        assert!(np > mean, "NP {np} should exceed mean {mean}");
        assert!(np < 300);
    }

    #[test]
    fn rolling_means_returns_expected_length() {
        let out = rolling_means(&[1, 2, 3, 4, 5], 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], 2); // mean(1,2,3)
        assert_eq!(out[1], 3);
        assert_eq!(out[2], 4);
    }
}
