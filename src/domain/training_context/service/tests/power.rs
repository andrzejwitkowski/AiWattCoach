use crate::domain::intervals::ActivityStream;

use super::super::{
    power::{compress_power_stream, extract_and_average_stream, extract_power_stream},
    MAX_CHUNKS_PER_WORKOUT,
};

#[test]
fn compressed_power_merges_identical_levels_into_runs() {
    assert_eq!(
        compress_power_stream(&[300, 300, 300], Some(300)),
        vec!["100:3"]
    );
}

#[test]
fn compressed_power_smooths_single_second_spike_outside_ftp_zone() {
    assert_eq!(
        compress_power_stream(&[210, 214, 210], Some(300)),
        vec!["41:3"]
    );
}

#[test]
fn compressed_power_merges_single_second_change_within_same_power_bucket() {
    assert_eq!(
        compress_power_stream(&[287, 290, 287], Some(300)),
        vec!["92:3"]
    );
}

#[test]
fn compressed_power_merges_boundary_change_when_bucketed_level_matches() {
    assert_eq!(
        compress_power_stream(&[286, 287, 286], Some(300)),
        vec!["92:3"]
    );
}

#[test]
fn compressed_power_buckets_watts_to_nearest_ten_before_encoding() {
    assert_eq!(
        compress_power_stream(&[284, 286, 289], Some(300)),
        vec!["84:1", "92:2"]
    );
}

#[test]
fn compressed_power_applies_run_cap_for_noisy_streams() {
    let noisy = (0..(MAX_CHUNKS_PER_WORKOUT * 2 + 1))
        .map(|index| if index % 2 == 0 { 300 } else { 0 })
        .collect::<Vec<_>>();

    assert!(compress_power_stream(&noisy, Some(300)).len() <= MAX_CHUNKS_PER_WORKOUT);
}

#[test]
fn compressed_power_run_cap_preserves_total_duration_seconds() {
    let noisy = (0..(MAX_CHUNKS_PER_WORKOUT * 2 + 1))
        .map(|index| if index % 2 == 0 { 300 } else { 0 })
        .collect::<Vec<_>>();

    let encoded = compress_power_stream(&noisy, Some(300));

    assert_eq!(sum_encoded_durations(&encoded), noisy.len());
}

#[test]
fn compressed_power_returns_empty_without_valid_ftp() {
    assert!(compress_power_stream(&[300, 300, 300], None).is_empty());
    assert!(compress_power_stream(&[300, 300, 300], Some(0)).is_empty());
}

#[test]
fn compressed_power_preserves_missing_watts_samples_as_zero_second_runs() {
    let streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: None,
        data: Some(serde_json::json!([200, null, 210])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];

    let encoded = compress_power_stream(&extract_power_stream(&streams), Some(300));

    assert_eq!(encoded, vec!["36:1", "0:1", "41:1"]);
    assert_eq!(sum_encoded_durations(&encoded), 3);
}

#[test]
fn extract_and_average_stream_preserves_missing_samples_for_alignment() {
    let streams = vec![ActivityStream {
        stream_type: "cadence".to_string(),
        name: None,
        data: Some(serde_json::json!([80, null, 84])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];

    assert_eq!(extract_and_average_stream(&streams, "cadence"), vec![55]);
}

fn sum_encoded_durations(runs: &[String]) -> usize {
    runs.iter()
        .map(|run| {
            run.split_once(':')
                .expect("encoded power run should contain level and duration")
                .1
                .parse::<usize>()
                .expect("encoded power duration should parse as usize")
        })
        .sum()
}
