use super::{CompletedWorkout, CompletedWorkoutPowerCurve, CompletedWorkoutSeries};

/// Errors that can occur when computing a power curve from workout data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerCurveError {
    InvalidResolution,
    DetailsUnavailable,
    WattsStreamMissing,
    WattsStreamUnsupportedType,
    NoValidPowerSamples,
    InsufficientData,
}

impl std::fmt::Display for PowerCurveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidResolution => write!(f, "invalid resolution"),
            Self::DetailsUnavailable => write!(f, "workout details are unavailable"),
            Self::WattsStreamMissing => write!(f, "no watts power stream in workout details"),
            Self::WattsStreamUnsupportedType => {
                write!(f, "watts stream uses an unsupported series type")
            }
            Self::NoValidPowerSamples => write!(f, "no valid power samples available"),
            Self::InsufficientData => write!(f, "not enough data for the requested resolution"),
        }
    }
}

impl std::error::Error for PowerCurveError {}

/// Computes the mean-max power curve for a completed workout.
///
/// Validates that `resolution_seconds` is a multiple of 5 (minimum 5), extracts
/// the watts stream, normalizes samples (negative, NaN, and infinite values are
/// treated as invalid gaps), and then uses prefix sums to compute the maximum
/// average power over sliding windows of each duration step.
///
/// Returns a [`CompletedWorkoutPowerCurve`] with one entry per duration step.
pub fn compute_power_curve(
    workout: &CompletedWorkout,
    resolution_seconds: u16,
) -> Result<CompletedWorkoutPowerCurve, PowerCurveError> {
    if resolution_seconds < 5 || resolution_seconds % 5 != 0 {
        return Err(PowerCurveError::InvalidResolution);
    }

    if workout.details_unavailable_reason.is_some() {
        return Err(PowerCurveError::DetailsUnavailable);
    }

    let watts_stream = workout
        .details
        .streams
        .iter()
        .find(|s| s.stream_type.to_lowercase() == "watts")
        .ok_or(PowerCurveError::WattsStreamMissing)?;

    let power_series = watts_stream
        .primary_series
        .as_ref()
        .or(watts_stream.secondary_series.as_ref());
    let power_samples = normalize_power_series(power_series)?;

    if power_samples.is_empty() || power_samples.iter().all(Option::is_none) {
        return Err(PowerCurveError::NoValidPowerSamples);
    }

    let source_samples = power_samples.len();
    if (resolution_seconds as usize) > source_samples {
        return Err(PowerCurveError::InsufficientData);
    }
    let valid_power_samples = power_samples.iter().filter(|v| v.is_some()).count();

    let (sum_prefix, valid_prefix) = build_prefixes(&power_samples);

    let max_duration = source_samples as u32;
    let step = resolution_seconds as u32;
    let num_points = (max_duration / step) as usize;

    let mut max_average_watts: Vec<Option<i32>> = Vec::with_capacity(num_points);

    for duration_multiple in 1..=num_points {
        let duration = (duration_multiple as u32 * step) as usize;
        let mut best: Option<i32> = None;

        for start in 0..=source_samples.saturating_sub(duration) {
            let end = start + duration;
            if valid_prefix[end] - valid_prefix[start] == duration {
                let sum = sum_prefix[end] - sum_prefix[start];
                let avg = (sum / duration as i64) as i32;
                best = Some(best.map_or(avg, |b| b.max(avg)));
            }
        }

        max_average_watts.push(best);
    }

    Ok(CompletedWorkoutPowerCurve {
        resolution_seconds,
        sample_period_seconds: 1,
        source_samples,
        valid_power_samples,
        duration_start_seconds: resolution_seconds as u32,
        duration_step_seconds: resolution_seconds,
        max_average_watts,
    })
}

fn normalize_power_series(
    series: Option<&CompletedWorkoutSeries>,
) -> Result<Vec<Option<i32>>, PowerCurveError> {
    match series {
        Some(CompletedWorkoutSeries::Integers(values)) => Ok(values
            .iter()
            .map(|&v| (v >= 0).then_some(v as i32))
            .collect()),
        Some(CompletedWorkoutSeries::Floats(values)) => Ok(values
            .iter()
            .map(|&v| {
                if v.is_finite() && v >= 0.0 && v <= i32::MAX as f64 {
                    Some(v.round() as i32)
                } else {
                    None
                }
            })
            .collect()),
        _ => Err(PowerCurveError::WattsStreamUnsupportedType),
    }
}

fn build_prefixes(samples: &[Option<i32>]) -> (Vec<i64>, Vec<usize>) {
    let n = samples.len();
    let mut sum_prefix = Vec::with_capacity(n + 1);
    let mut valid_prefix = Vec::with_capacity(n + 1);
    sum_prefix.push(0);
    valid_prefix.push(0);

    let mut running_sum: i64 = 0;
    let mut running_valid: usize = 0;
    for sample in samples {
        if let Some(value) = sample {
            running_sum += *value as i64;
            running_valid += 1;
        }
        sum_prefix.push(running_sum);
        valid_prefix.push(running_valid);
    }

    (sum_prefix, valid_prefix)
}
