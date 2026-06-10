use crate::domain::{intervals::ActivityStream, workout_streams};

pub(super) fn extract_and_average_stream(
    streams: &[ActivityStream],
    stream_type: &str,
    bucket_size: usize,
) -> Vec<i32> {
    let values = extract_raw_stream(streams, stream_type);
    workout_streams::average_into_buckets(&values, bucket_size)
}

pub(super) fn extract_power_values_3s(streams: &[ActivityStream]) -> Vec<i32> {
    extract_and_average_stream(streams, "watts", workout_streams::POWER_BUCKET_SECONDS)
}

fn extract_raw_stream(streams: &[ActivityStream], stream_type: &str) -> Vec<i32> {
    streams
        .iter()
        .find(|stream| stream.stream_type.eq_ignore_ascii_case(stream_type))
        .and_then(|stream| stream.data.as_ref())
        .map(extract_numeric_values)
        .unwrap_or_default()
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
