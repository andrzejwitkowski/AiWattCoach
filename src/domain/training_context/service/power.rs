use crate::domain::intervals::ActivityStream;

use super::{MAX_CHUNKS_PER_WORKOUT, STREAM_BUCKET_SIZE};

const POWER_BUCKET_WATTS: i32 = 10;

pub(super) fn extract_and_average_stream(
    streams: &[ActivityStream],
    stream_type: &str,
) -> Vec<i32> {
    let values = extract_raw_stream(streams, stream_type);

    let chunks = values
        .chunks(STREAM_BUCKET_SIZE)
        .map(|chunk| (chunk.iter().sum::<i32>() as f64 / chunk.len() as f64).round() as i32)
        .collect::<Vec<_>>();

    compress_stream_chunks(chunks)
}

fn extract_raw_stream(streams: &[ActivityStream], stream_type: &str) -> Vec<i32> {
    streams
        .iter()
        .find(|stream| stream.stream_type.eq_ignore_ascii_case(stream_type))
        .and_then(|stream| stream.data.as_ref())
        .map(extract_numeric_values)
        .unwrap_or_default()
}

pub(super) fn extract_power_stream(streams: &[ActivityStream]) -> Vec<i32> {
    streams
        .iter()
        .find(|stream| stream.stream_type.eq_ignore_ascii_case("watts"))
        .and_then(|stream| stream.data.as_ref())
        .map(extract_power_values)
        .unwrap_or_default()
}

pub(super) fn compress_power_stream(values: &[i32], ftp_watts: Option<i32>) -> Vec<String> {
    let Some(ftp_watts) = ftp_watts.filter(|value| *value > 0) else {
        return Vec::new();
    };

    let mut levels = values
        .iter()
        .map(|value| encode_power_level(*value, ftp_watts))
        .collect::<Vec<_>>();

    smooth_single_second_level_noise(&mut levels);
    compress_encoded_runs(run_length_encode_levels(&levels))
}

fn encode_power_level(power_watts: i32, ftp_watts: i32) -> i32 {
    if power_watts <= 0 {
        return 0;
    }

    let power_watts = round_to_nearest_power_bucket(power_watts);

    ((power_watts as f64 / ftp_watts as f64).powf(2.5) * 100.0).round() as i32
}

fn round_to_nearest_power_bucket(power_watts: i32) -> i32 {
    let half_bucket = POWER_BUCKET_WATTS / 2;
    ((power_watts + half_bucket) / POWER_BUCKET_WATTS) * POWER_BUCKET_WATTS
}

fn smooth_single_second_level_noise(levels: &mut [i32]) {
    if levels.len() < 3 {
        return;
    }

    for index in 1..(levels.len() - 1) {
        let previous = levels[index - 1];
        let current = levels[index];
        let next = levels[index + 1];

        if previous != next {
            continue;
        }

        if (current - previous).abs() >= 3 {
            continue;
        }

        if (90..=110).contains(&previous) || (90..=110).contains(&current) {
            continue;
        }

        levels[index] = previous;
    }
}

fn run_length_encode_levels(levels: &[i32]) -> Vec<String> {
    let Some((&first, rest)) = levels.split_first() else {
        return Vec::new();
    };

    let mut encoded = Vec::new();
    let mut current_level = first;
    let mut duration_seconds = 1;

    for &level in rest {
        if level == current_level {
            duration_seconds += 1;
            continue;
        }

        encoded.push(format!("{current_level}:{duration_seconds}"));
        current_level = level;
        duration_seconds = 1;
    }

    encoded.push(format!("{current_level}:{duration_seconds}"));
    encoded
}

#[derive(Clone, Copy)]
struct EncodedPowerRun {
    level: i32,
    duration_seconds: usize,
}

fn compress_encoded_runs(runs: Vec<String>) -> Vec<String> {
    if runs.len() <= MAX_CHUNKS_PER_WORKOUT {
        return runs;
    }

    let runs = parse_encoded_runs(runs);
    let recent_count = MAX_CHUNKS_PER_WORKOUT / 2;
    let summary_count = MAX_CHUNKS_PER_WORKOUT - recent_count;
    let older_count = runs.len() - recent_count;
    let group_size = older_count.div_ceil(summary_count);
    let summarized = runs[..older_count]
        .chunks(group_size)
        .map(summarize_encoded_run_group);

    format_encoded_runs(merge_adjacent_runs(
        summarized
            .chain(runs[older_count..].iter().copied())
            .collect::<Vec<_>>(),
    ))
}

fn compress_stream_chunks(chunks: Vec<i32>) -> Vec<i32> {
    if chunks.len() <= MAX_CHUNKS_PER_WORKOUT {
        return chunks;
    }

    let recent_count = MAX_CHUNKS_PER_WORKOUT / 2;
    let summary_count = MAX_CHUNKS_PER_WORKOUT - recent_count;
    let older_count = chunks.len() - recent_count;
    let group_size = older_count.div_ceil(summary_count);
    let summarized = chunks[..older_count]
        .chunks(group_size)
        .map(|group| (group.iter().sum::<i32>() as f64 / group.len() as f64).round() as i32);

    summarized
        .chain(chunks[older_count..].iter().copied())
        .collect()
}

fn parse_encoded_runs(runs: Vec<String>) -> Vec<EncodedPowerRun> {
    runs.into_iter()
        .map(|run| {
            let (level, duration) = run
                .split_once(':')
                .expect("encoded power run should contain level and duration");
            EncodedPowerRun {
                level: level
                    .parse::<i32>()
                    .expect("encoded power level should parse as i32"),
                duration_seconds: duration
                    .parse::<usize>()
                    .expect("encoded power duration should parse as usize"),
            }
        })
        .collect()
}

fn summarize_encoded_run_group(group: &[EncodedPowerRun]) -> EncodedPowerRun {
    let total_duration_seconds = group.iter().map(|run| run.duration_seconds).sum::<usize>();
    let weighted_level_sum = group
        .iter()
        .map(|run| run.level as f64 * run.duration_seconds as f64)
        .sum::<f64>();

    EncodedPowerRun {
        level: (weighted_level_sum / total_duration_seconds as f64).round() as i32,
        duration_seconds: total_duration_seconds,
    }
}

fn merge_adjacent_runs(runs: Vec<EncodedPowerRun>) -> Vec<EncodedPowerRun> {
    let mut merged: Vec<EncodedPowerRun> = Vec::with_capacity(runs.len());

    for run in runs {
        if let Some(previous) = merged.last_mut() {
            if previous.level == run.level {
                previous.duration_seconds += run.duration_seconds;
                continue;
            }
        }

        merged.push(run);
    }

    merged
}

fn format_encoded_runs(runs: Vec<EncodedPowerRun>) -> Vec<String> {
    runs.into_iter()
        .map(|run| format!("{}:{}", run.level, run.duration_seconds))
        .collect()
}

fn extract_numeric_values(value: &serde_json::Value) -> Vec<i32> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .unwrap_or(0)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_power_values(value: &serde_json::Value) -> Vec<i32> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .unwrap_or(0)
                })
                .collect()
        })
        .unwrap_or_default()
}
