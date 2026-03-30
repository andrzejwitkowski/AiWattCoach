use super::{ParsedWorkoutDoc, WorkoutIntervalDefinition, WorkoutSegment, WorkoutSummary};

const MAX_PARSED_SEGMENTS: usize = 10_000;

pub fn parse_workout_doc(workout_doc: Option<&str>, ftp_watts: Option<i32>) -> ParsedWorkoutDoc {
    let intervals = workout_doc
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_workout_line)
        .collect::<Vec<_>>();

    let mut segments = Vec::new();
    let mut start_offset_seconds: i32 = 0;

    'intervals: for interval in &intervals {
        let Some(duration_seconds) = interval.duration_seconds else {
            continue;
        };

        for repeat_index in 0..interval.repeat_count {
            if segments.len() >= MAX_PARSED_SEGMENTS {
                break 'intervals;
            }

            let label = if interval.repeat_count > 1 {
                format!(
                    "{} #{}",
                    normalize_definition(&interval.definition),
                    repeat_index + 1
                )
            } else {
                normalize_definition(&interval.definition)
            };

            let Some(end_offset_seconds) = start_offset_seconds.checked_add(duration_seconds)
            else {
                break 'intervals;
            };
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

    Some(super::round_to(percent, 1))
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
    let estimated_intensity_factor = super::round_to(normalized_intensity, 3);
    let estimated_training_stress_score = super::round_to(
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
