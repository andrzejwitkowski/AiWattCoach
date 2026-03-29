use serde::{Deserialize, Serialize};

use super::{Activity, ActivityInterval};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedWorkoutDoc {
    pub intervals: Vec<WorkoutIntervalDefinition>,
    pub segments: Vec<WorkoutSegment>,
    pub summary: WorkoutSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutIntervalDefinition {
    pub definition: String,
    pub repeat_count: usize,
    pub duration_seconds: Option<i32>,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSegment {
    pub order: usize,
    pub label: String,
    pub duration_seconds: i32,
    pub start_offset_seconds: i32,
    pub end_offset_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSummary {
    pub total_segments: usize,
    pub total_duration_seconds: i32,
    pub estimated_normalized_power_watts: Option<i32>,
    pub estimated_average_power_watts: Option<i32>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_training_stress_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActualWorkoutMatch {
    pub activity_id: String,
    pub activity_name: Option<String>,
    pub start_date_local: String,
    pub power_values: Vec<i32>,
    pub cadence_values: Vec<i32>,
    pub heart_rate_values: Vec<i32>,
    pub speed_values: Vec<f64>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub training_stress_score: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub compliance_score: f64,
    pub matched_intervals: Vec<MatchedWorkoutInterval>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchedWorkoutInterval {
    pub planned_segment_order: usize,
    pub planned_label: String,
    pub planned_duration_seconds: i32,
    pub target_percent_ftp: Option<f64>,
    pub zone_id: Option<i32>,
    pub actual_interval_id: Option<i32>,
    pub actual_start_time_seconds: Option<i32>,
    pub actual_end_time_seconds: Option<i32>,
    pub average_power_watts: Option<i32>,
    pub normalized_power_watts: Option<i32>,
    pub average_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub compliance_score: f64,
}

pub fn parse_workout_doc(workout_doc: Option<&str>, ftp_watts: Option<i32>) -> ParsedWorkoutDoc {
    let intervals = workout_doc
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_workout_line)
        .collect::<Vec<_>>();

    let mut segments = Vec::new();
    let mut start_offset_seconds = 0;

    for interval in &intervals {
        let Some(duration_seconds) = interval.duration_seconds else {
            continue;
        };

        for repeat_index in 0..interval.repeat_count {
            let label = if interval.repeat_count > 1 {
                format!(
                    "{} #{}",
                    normalize_definition(&interval.definition),
                    repeat_index + 1
                )
            } else {
                normalize_definition(&interval.definition)
            };

            let end_offset_seconds = start_offset_seconds + duration_seconds;
            segments.push(WorkoutSegment {
                order: segments.len(),
                label,
                duration_seconds,
                start_offset_seconds,
                end_offset_seconds,
                target_percent_ftp: interval.target_percent_ftp,
                zone_id: interval.zone_id,
            });
            start_offset_seconds = end_offset_seconds;
        }
    }

    let summary = build_workout_summary(&segments, ftp_watts);

    ParsedWorkoutDoc {
        intervals,
        segments,
        summary,
    }
}

pub fn find_best_activity_match(
    parsed: &ParsedWorkoutDoc,
    activities: &[Activity],
    ftp_watts: Option<i32>,
) -> Option<ActualWorkoutMatch> {
    let planned_segments = matching_segments(parsed);
    if planned_segments.is_empty() {
        return None;
    }

    activities
        .iter()
        .filter_map(|activity| evaluate_activity_match(activity, &planned_segments, ftp_watts))
        .max_by(|left, right| left.compliance_score.total_cmp(&right.compliance_score))
        .filter(|matched| matched.compliance_score >= 0.45)
}

fn parse_workout_line(definition: &str) -> WorkoutIntervalDefinition {
    let normalized_definition = definition.trim().to_string();
    let clean = normalize_definition(definition);
    let tokens = clean.split_whitespace().collect::<Vec<_>>();
    let (repeat_count, duration_seconds) = parse_repeat_and_duration(&tokens).unwrap_or((1, None));
    let target_percent_ftp = tokens.iter().find_map(|token| parse_target_percent(token));
    let zone_id = tokens.iter().find_map(|token| parse_zone_token(token));
    let target_percent_ftp = target_percent_ftp.or_else(|| zone_id.map(percent_for_zone));

    WorkoutIntervalDefinition {
        definition: normalized_definition,
        repeat_count,
        duration_seconds,
        target_percent_ftp,
        zone_id: zone_id.or_else(|| target_percent_ftp.map(zone_for_percent)),
    }
}

fn parse_repeat_and_duration(tokens: &[&str]) -> Option<(usize, Option<i32>)> {
    let first = tokens.first()?.trim().to_ascii_lowercase();
    if let Some((repeat_count, duration_token)) = first.split_once('x') {
        if let Ok(repeat_count) = repeat_count.parse::<usize>() {
            if let Some(duration_seconds) = parse_duration_token(duration_token) {
                return Some((repeat_count.max(1), Some(duration_seconds)));
            }

            if duration_token.is_empty() {
                let next_duration = tokens.get(1).and_then(|token| parse_duration_token(token));
                return Some((repeat_count.max(1), next_duration));
            }
        }
    }

    Some((
        1,
        tokens.first().and_then(|token| parse_duration_token(token)),
    ))
}

fn parse_duration_token(token: &str) -> Option<i32> {
    let token = token.trim().trim_end_matches(',').to_ascii_lowercase();
    let split_index = token
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(token.len());
    let (value, unit) = token.split_at(split_index);
    if value.is_empty() || unit.is_empty() {
        return None;
    }

    let value = value.parse::<f64>().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }

    let seconds = match unit {
        "h" | "hr" | "hrs" | "hour" | "hours" => value * 3600.0,
        "m" | "min" | "mins" | "minute" | "minutes" => value * 60.0,
        "s" | "sec" | "secs" | "second" | "seconds" => value,
        _ => return None,
    };

    Some(seconds.round() as i32)
}

fn parse_target_percent(token: &str) -> Option<f64> {
    let value = token.trim().trim_end_matches(',');
    if !value.ends_with('%') {
        return None;
    }

    let raw = value.trim_end_matches('%');
    let percent = if let Some((start, end)) = raw.split_once('-') {
        let start = start.parse::<f64>().ok()?;
        let end = end.parse::<f64>().ok()?;
        (start + end) / 2.0
    } else {
        raw.parse::<f64>().ok()?
    };

    if !percent.is_finite() || percent <= 0.0 {
        return None;
    }

    Some(round_to(percent, 1))
}

fn parse_zone_token(token: &str) -> Option<i32> {
    let normalized = token.trim().trim_end_matches(',').to_ascii_lowercase();
    let zone = normalized.strip_prefix('z')?.parse::<i32>().ok()?;
    (1..=7).contains(&zone).then_some(zone)
}

fn normalize_definition(definition: &str) -> String {
    definition.trim().trim_start_matches('-').trim().to_string()
}

fn zone_for_percent(percent: f64) -> i32 {
    match percent {
        value if value <= 55.0 => 1,
        value if value < 76.0 => 2,
        value if value < 91.0 => 3,
        value if value < 106.0 => 4,
        value if value < 121.0 => 5,
        value if value < 151.0 => 6,
        _ => 7,
    }
}

fn percent_for_zone(zone_id: i32) -> f64 {
    match zone_id {
        1 => 50.0,
        2 => 65.0,
        3 => 83.0,
        4 => 98.0,
        5 => 113.0,
        6 => 130.0,
        _ => 160.0,
    }
}

fn build_workout_summary(segments: &[WorkoutSegment], ftp_watts: Option<i32>) -> WorkoutSummary {
    let total_duration_seconds = segments
        .iter()
        .map(|segment| segment.duration_seconds)
        .sum::<i32>();
    let total_segments = segments.len();

    if total_duration_seconds <= 0
        || segments
            .iter()
            .any(|segment| segment.target_percent_ftp.is_none())
    {
        return WorkoutSummary {
            total_segments,
            total_duration_seconds,
            estimated_normalized_power_watts: None,
            estimated_average_power_watts: None,
            estimated_intensity_factor: None,
            estimated_training_stress_score: None,
        };
    }

    let total_duration = total_duration_seconds as f64;
    let average_intensity = segments
        .iter()
        .map(|segment| {
            (segment.duration_seconds as f64)
                * (segment.target_percent_ftp.unwrap_or_default() / 100.0)
        })
        .sum::<f64>()
        / total_duration;
    let normalized_intensity = (segments
        .iter()
        .map(|segment| {
            let intensity = segment.target_percent_ftp.unwrap_or_default() / 100.0;
            (segment.duration_seconds as f64) * intensity.powi(4)
        })
        .sum::<f64>()
        / total_duration)
        .powf(0.25);
    let estimated_intensity_factor = round_to(normalized_intensity, 3);
    let estimated_training_stress_score = round_to(
        (total_duration_seconds as f64 / 3600.0) * estimated_intensity_factor.powi(2) * 100.0,
        1,
    );
    let ftp_watts = ftp_watts.filter(|value| *value > 0);
    let estimated_average_power_watts =
        ftp_watts.map(|ftp_watts| (average_intensity * ftp_watts as f64).round() as i32);
    let estimated_normalized_power_watts =
        ftp_watts.map(|ftp_watts| (normalized_intensity * ftp_watts as f64).round() as i32);

    WorkoutSummary {
        total_segments,
        total_duration_seconds,
        estimated_normalized_power_watts,
        estimated_average_power_watts,
        estimated_intensity_factor: Some(estimated_intensity_factor),
        estimated_training_stress_score: Some(estimated_training_stress_score),
    }
}

fn matching_segments(parsed: &ParsedWorkoutDoc) -> Vec<WorkoutSegment> {
    let work_segments = parsed
        .segments
        .iter()
        .filter(|segment| segment.target_percent_ftp.unwrap_or_default() >= 75.0)
        .cloned()
        .collect::<Vec<_>>();

    if work_segments.is_empty() {
        parsed
            .segments
            .iter()
            .filter(|segment| segment.target_percent_ftp.is_some())
            .cloned()
            .collect()
    } else {
        work_segments
    }
}

fn evaluate_activity_match(
    activity: &Activity,
    planned_segments: &[WorkoutSegment],
    ftp_watts: Option<i32>,
) -> Option<ActualWorkoutMatch> {
    let power_values = extract_integer_stream(activity, &["watts"]);
    let cadence_values = extract_integer_stream(activity, &["cadence"]);
    let heart_rate_values = extract_integer_stream(activity, &["heartrate", "heart_rate"]);
    let speed_values = extract_float_stream(activity, &["velocity_smooth", "speed"]);

    let detected_intervals = detected_interval_candidates(activity);
    let fallback_intervals = if detected_intervals.is_empty() {
        detect_intervals_from_power_stream(planned_segments, &power_values, ftp_watts)
    } else {
        Vec::new()
    };
    let actual_intervals = if detected_intervals.is_empty() {
        fallback_intervals
    } else {
        detected_intervals
    };

    let pair_count = planned_segments.len().min(actual_intervals.len());
    if pair_count == 0 {
        return None;
    }

    let matched_intervals = planned_segments
        .iter()
        .zip(actual_intervals.iter())
        .map(|(planned, actual)| {
            let expected_watts = ftp_watts.and_then(|ftp| {
                planned
                    .target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            });
            let duration_score = similarity_score(
                actual.duration_seconds as f64,
                planned.duration_seconds as f64,
            );
            let power_score = match (expected_watts, actual.average_power_watts) {
                (Some(expected), Some(actual_power)) => {
                    similarity_score(actual_power as f64, expected as f64)
                }
                _ => 0.5,
            };
            let zone_score = match (planned.zone_id, actual.zone_id) {
                (Some(planned_zone), Some(actual_zone)) => {
                    (1.0 - ((planned_zone - actual_zone).abs() as f64 / 6.0)).clamp(0.0, 1.0)
                }
                _ => 0.5,
            };
            let compliance_score = round_to(
                (power_score * 0.55) + (duration_score * 0.35) + (zone_score * 0.10),
                3,
            );

            MatchedWorkoutInterval {
                planned_segment_order: planned.order,
                planned_label: planned.label.clone(),
                planned_duration_seconds: planned.duration_seconds,
                target_percent_ftp: planned.target_percent_ftp,
                zone_id: planned.zone_id,
                actual_interval_id: actual.id,
                actual_start_time_seconds: Some(actual.start_time_seconds),
                actual_end_time_seconds: Some(actual.end_time_seconds),
                average_power_watts: actual.average_power_watts,
                normalized_power_watts: actual.normalized_power_watts,
                average_heart_rate_bpm: actual.average_heart_rate_bpm,
                average_cadence_rpm: actual.average_cadence_rpm,
                average_speed_mps: actual.average_speed_mps,
                compliance_score,
            }
        })
        .collect::<Vec<_>>();

    let pair_score = matched_intervals
        .iter()
        .map(|interval| interval.compliance_score)
        .sum::<f64>()
        / matched_intervals.len() as f64;
    let count_score = pair_count as f64 / planned_segments.len() as f64;
    let compliance_score = round_to((pair_score * 0.8) + (count_score * 0.2), 3);

    Some(ActualWorkoutMatch {
        activity_id: activity.id.clone(),
        activity_name: activity.name.clone(),
        start_date_local: activity.start_date_local.clone(),
        power_values,
        cadence_values,
        heart_rate_values,
        speed_values,
        average_power_watts: activity.metrics.average_power_watts,
        normalized_power_watts: activity.metrics.normalized_power_watts,
        training_stress_score: activity.metrics.training_stress_score,
        intensity_factor: activity
            .metrics
            .intensity_factor
            .map(|value| round_to(value, 3)),
        compliance_score,
        matched_intervals,
    })
}

fn detected_interval_candidates(activity: &Activity) -> Vec<IntervalCandidate> {
    let preferred = activity
        .details
        .intervals
        .iter()
        .filter_map(IntervalCandidate::from_interval)
        .filter(|interval| {
            interval
                .interval_type
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("WORK"))
                .unwrap_or(false)
                || interval.zone_id.unwrap_or_default() >= 3
        })
        .collect::<Vec<_>>();

    if preferred.is_empty() {
        activity
            .details
            .intervals
            .iter()
            .filter_map(IntervalCandidate::from_interval)
            .collect()
    } else {
        preferred
    }
}

fn detect_intervals_from_power_stream(
    planned_segments: &[WorkoutSegment],
    power_values: &[i32],
    ftp_watts: Option<i32>,
) -> Vec<IntervalCandidate> {
    let Some(ftp_watts) = ftp_watts.filter(|value| *value > 0) else {
        return Vec::new();
    };
    if power_values.is_empty() {
        return Vec::new();
    }

    let prefix_sums = power_values.iter().fold(vec![0_i64], |mut sums, value| {
        let next = sums.last().copied().unwrap_or_default() + i64::from(*value);
        sums.push(next);
        sums
    });
    let mut search_start = 0usize;
    let mut detected = Vec::new();

    for planned in planned_segments {
        let Some(target_percent) = planned.target_percent_ftp else {
            continue;
        };
        let window = planned.duration_seconds.max(30) as usize;
        if search_start + window > power_values.len() {
            break;
        }

        let expected_watts = (ftp_watts as f64 * target_percent / 100.0).round();
        let mut best: Option<(usize, f64)> = None;
        for start in search_start..=(power_values.len() - window) {
            let end = start + window;
            let average = (prefix_sums[end] - prefix_sums[start]) as f64 / window as f64;
            let score = similarity_score(average, expected_watts);
            if best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score)
            {
                best = Some((start, score));
            }
        }

        let Some((start_index, _)) = best else {
            break;
        };
        let end_index = start_index + window;
        let average_power_watts = ((prefix_sums[end_index] - prefix_sums[start_index]) as f64
            / window as f64)
            .round() as i32;
        detected.push(IntervalCandidate {
            id: None,
            interval_type: Some("POWER_STREAM".to_string()),
            zone_id: planned.zone_id,
            start_time_seconds: start_index as i32,
            end_time_seconds: end_index as i32,
            duration_seconds: window as i32,
            average_power_watts: Some(average_power_watts),
            normalized_power_watts: Some(average_power_watts),
            average_heart_rate_bpm: None,
            average_cadence_rpm: None,
            average_speed_mps: None,
        });
        search_start = end_index;
    }

    detected
}

fn extract_integer_stream(activity: &Activity, stream_types: &[&str]) -> Vec<i32> {
    extract_float_stream(activity, stream_types)
        .into_iter()
        .map(|value| value.round() as i32)
        .collect()
}

fn extract_float_stream(activity: &Activity, stream_types: &[&str]) -> Vec<f64> {
    activity
        .details
        .streams
        .iter()
        .find(|stream| {
            stream_types
                .iter()
                .any(|stream_type| stream.stream_type.eq_ignore_ascii_case(stream_type))
        })
        .and_then(|stream| stream.data.as_ref())
        .and_then(|value| value.as_array().cloned())
        .map(|values| {
            values
                .into_iter()
                .filter_map(|value| value.as_f64())
                .collect()
        })
        .unwrap_or_default()
}

fn similarity_score(actual: f64, expected: f64) -> f64 {
    if expected <= 0.0 {
        return 0.0;
    }

    (1.0 - ((actual - expected).abs() / expected)).clamp(0.0, 1.0)
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

#[derive(Clone, Debug)]
struct IntervalCandidate {
    id: Option<i32>,
    interval_type: Option<String>,
    zone_id: Option<i32>,
    start_time_seconds: i32,
    end_time_seconds: i32,
    duration_seconds: i32,
    average_power_watts: Option<i32>,
    normalized_power_watts: Option<i32>,
    average_heart_rate_bpm: Option<i32>,
    average_cadence_rpm: Option<f64>,
    average_speed_mps: Option<f64>,
}

impl IntervalCandidate {
    fn from_interval(interval: &ActivityInterval) -> Option<Self> {
        let start_time_seconds = interval.start_time_seconds?;
        let end_time_seconds = interval.end_time_seconds?;
        let duration_seconds = (end_time_seconds - start_time_seconds).max(0);
        if duration_seconds <= 0 {
            return None;
        }

        Some(Self {
            id: interval.id,
            interval_type: interval.interval_type.clone(),
            zone_id: interval.zone,
            start_time_seconds,
            end_time_seconds,
            duration_seconds,
            average_power_watts: interval.average_power_watts,
            normalized_power_watts: interval.normalized_power_watts,
            average_heart_rate_bpm: interval.average_heart_rate_bpm,
            average_cadence_rpm: interval.average_cadence_rpm,
            average_speed_mps: interval.average_speed_mps,
        })
    }
}
