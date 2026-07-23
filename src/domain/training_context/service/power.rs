use crate::domain::{
    intervals::ActivityStream,
    workout_streams::{self, SegmentTriplet},
};

pub(super) fn extract_power_segments_3s(streams: &[ActivityStream]) -> Vec<SegmentTriplet> {
    let values = extract_raw_stream(streams, "watts");
    workout_streams::bucket_and_encode_power_segments(&values)
}

pub(super) fn extract_cadence_segments_5s(streams: &[ActivityStream]) -> Vec<SegmentTriplet> {
    let values = extract_raw_stream(streams, "cadence");
    workout_streams::bucket_and_encode_cadence_segments(&values)
}

fn extract_raw_stream(streams: &[ActivityStream], stream_type: &str) -> Vec<i32> {
    streams
        .iter()
        .find(|stream| stream.stream_type.eq_ignore_ascii_case(stream_type))
        .and_then(|stream| stream.data.as_ref())
        .map(extract_numeric_values)
        .unwrap_or_default()
}

/// Raw per-second samples for a stream type (no bucketing).
pub(super) fn raw_stream(streams: &[ActivityStream], stream_type: &str) -> Vec<i32> {
    extract_raw_stream(streams, stream_type)
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
