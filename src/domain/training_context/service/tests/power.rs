use crate::domain::intervals::ActivityStream;

use super::super::power::extract_and_average_stream;

#[test]
fn power_3s_averages_watts_into_three_second_buckets() {
    assert_eq!(
        extract_and_average_stream(&watts_stream(&[200, 220, 240, 260, 280]), "watts", 3,),
        vec![220, 270],
    );
}

#[test]
fn power_3s_returns_empty_without_watts_stream() {
    assert!(extract_and_average_stream(&[], "watts", 3).is_empty());
}

#[test]
fn power_3s_returns_full_bucket_count_for_long_streams() {
    let values: Vec<i32> = (0..1000).collect();
    assert_eq!(
        extract_and_average_stream(&watts_stream(&values), "watts", 3).len(),
        334
    );
}

#[test]
fn power_3s_preserves_missing_samples_as_zero_in_bucket_average() {
    let streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: None,
        data: Some(serde_json::json!([200, null, 210])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];
    assert_eq!(extract_and_average_stream(&streams, "watts", 3), vec![137]);
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

    assert_eq!(extract_and_average_stream(&streams, "cadence", 5), vec![55]);
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
