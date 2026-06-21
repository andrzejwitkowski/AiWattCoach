use crate::domain::intervals::ActivityStream;

use super::super::power::{extract_cadence_segments_5s, extract_power_segments_3s};

#[test]
fn power_segments_encode_fixture_watts() {
    assert_eq!(
        extract_power_segments_3s(&watts_stream(&[200, 220, 240, 260, 280])),
        vec![[220, 220, 3], [270, 270, 3]],
    );
}

#[test]
fn power_segments_return_empty_without_watts_stream() {
    assert!(extract_power_segments_3s(&[]).is_empty());
}

#[test]
fn power_segments_compress_long_steady_stream() {
    let values: Vec<i32> = vec![245; 7200];
    let segments = extract_power_segments_3s(&watts_stream(&values));
    assert_eq!(segments.len(), 1);
    assert!(segments[0][2] >= 7200);
}

#[test]
fn power_segments_preserve_missing_samples_as_zero_in_bucket_average() {
    let streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: None,
        data: Some(serde_json::json!([200, null, 210])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];
    assert_eq!(extract_power_segments_3s(&streams), vec![[137, 137, 3]]);
}

#[test]
fn cadence_segments_encode_stream() {
    let streams = vec![ActivityStream {
        stream_type: "cadence".to_string(),
        name: None,
        data: Some(serde_json::json!([80, null, 84])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];

    assert_eq!(extract_cadence_segments_5s(&streams), vec![[55, 55, 5]]);
}

#[test]
fn cadence_segments_compress_long_steady_stream() {
    let values: Vec<i32> = vec![88; 7200];
    let segments = extract_cadence_segments_5s(&cadence_stream(&values));
    assert_eq!(segments.len(), 1);
    assert!(segments[0][2] >= 7200);
}

fn cadence_stream(values: &[i32]) -> Vec<ActivityStream> {
    vec![ActivityStream {
        stream_type: "cadence".to_string(),
        name: None,
        data: Some(serde_json::json!(values)),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }]
}

fn watts_stream(values: &[i32]) -> Vec<ActivityStream> {
    vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: None,
        data: Some(serde_json::json!(values)),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }]
}
