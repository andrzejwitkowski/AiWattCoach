pub const POWER_TOLERANCE_WATTS: i32 = 5;
pub const POWER_COASTING_THRESHOLD_WATTS: i32 = 10;
pub const CADENCE_TOLERANCE_RPM: i32 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StreamSegment {
    min: i32,
    max: i32,
    duration_seconds: i32,
}

pub type SegmentTriplet = [i32; 3];

pub fn bucket_and_encode_power_segments(values: &[i32]) -> Vec<SegmentTriplet> {
    let buckets = super::average_into_buckets(values, super::POWER_BUCKET_SECONDS);
    encode_segments(
        &buckets,
        super::POWER_BUCKET_SECONDS,
        POWER_TOLERANCE_WATTS,
        true,
    )
    .into_iter()
    .map(|segment| [segment.min, segment.max, segment.duration_seconds])
    .collect()
}

pub fn bucket_and_encode_cadence_segments(values: &[i32]) -> Vec<SegmentTriplet> {
    let buckets = super::average_into_buckets(values, super::CADENCE_BUCKET_SECONDS);
    encode_segments(
        &buckets,
        super::CADENCE_BUCKET_SECONDS,
        CADENCE_TOLERANCE_RPM,
        false,
    )
    .into_iter()
    .map(|segment| [segment.min, segment.max, segment.duration_seconds])
    .collect()
}

fn encode_segments(
    buckets: &[i32],
    bucket_seconds: usize,
    tolerance: i32,
    check_coasting_boundary: bool,
) -> Vec<StreamSegment> {
    if buckets.is_empty() || bucket_seconds == 0 {
        return Vec::new();
    }

    let bucket_duration = i32::try_from(bucket_seconds).unwrap_or(i32::MAX);
    let mut segments = Vec::new();
    let mut current = StreamSegment {
        min: buckets[0],
        max: buckets[0],
        duration_seconds: bucket_duration,
    };

    for &value in &buckets[1..] {
        if can_extend_segment(&current, value, tolerance, check_coasting_boundary) {
            current.min = current.min.min(value);
            current.max = current.max.max(value);
            current.duration_seconds += bucket_duration;
        } else {
            segments.push(current);
            current = StreamSegment {
                min: value,
                max: value,
                duration_seconds: bucket_duration,
            };
        }
    }

    segments.push(current);
    segments
}

fn can_extend_segment(
    segment: &StreamSegment,
    value: i32,
    tolerance: i32,
    check_coasting_boundary: bool,
) -> bool {
    if check_coasting_boundary && crosses_power_coasting_boundary(segment.max, value) {
        return false;
    }

    let expanded_min = segment.min - tolerance;
    let expanded_max = segment.max + tolerance;
    (expanded_min..=expanded_max).contains(&value)
}

fn crosses_power_coasting_boundary(segment_max: i32, value: i32) -> bool {
    let segment_is_coasting = segment_max < POWER_COASTING_THRESHOLD_WATTS;
    let value_is_coasting = value < POWER_COASTING_THRESHOLD_WATTS;
    segment_is_coasting != value_is_coasting
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_power_segments(buckets: &[i32], bucket_seconds: usize) -> Vec<StreamSegment> {
        encode_segments(buckets, bucket_seconds, POWER_TOLERANCE_WATTS, true)
    }

    fn encode_cadence_segments(buckets: &[i32], bucket_seconds: usize) -> Vec<StreamSegment> {
        encode_segments(buckets, bucket_seconds, CADENCE_TOLERANCE_RPM, false)
    }

    #[test]
    fn steady_power_collapses_to_single_segment() {
        let buckets = vec![250; 100];
        let segments = encode_power_segments(&buckets, 3);
        assert_eq!(
            segments,
            vec![StreamSegment {
                min: 250,
                max: 250,
                duration_seconds: 300,
            }]
        );
    }

    #[test]
    fn interval_workout_splits_on_off_segments() {
        let mut buckets = vec![50; 20];
        buckets.extend(vec![300; 40]);
        buckets.extend(vec![50; 20]);
        let segments = encode_power_segments(&buckets, 3);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].max, 50);
        assert_eq!(segments[1].min, 300);
        assert_eq!(segments[1].max, 300);
        assert_eq!(segments[2].max, 50);
    }

    #[test]
    fn coasting_not_merged_with_work() {
        let buckets = vec![0, 0, 250, 250, 250];
        let segments = encode_power_segments(&buckets, 3);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].max, 0);
        assert_eq!(segments[1].min, 250);
    }

    #[test]
    fn short_spike_preserved_when_outside_tolerance() {
        let buckets = vec![250, 250, 310, 250, 250];
        let segments = encode_power_segments(&buckets, 3);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[1].min, 310);
        assert_eq!(segments[1].max, 310);
    }

    #[test]
    fn cadence_steady_block_merges_within_tolerance() {
        let buckets = vec![85, 86, 87, 88];
        let segments = encode_cadence_segments(&buckets, 5);
        assert_eq!(
            segments,
            vec![StreamSegment {
                min: 85,
                max: 88,
                duration_seconds: 20,
            }]
        );
    }

    #[test]
    fn bucket_and_encode_power_fixture_matches_expected_shape() {
        let triplets = bucket_and_encode_power_segments(&[200, 220, 240, 260, 280]);
        assert_eq!(triplets, vec![[220, 220, 3], [270, 270, 3]]);
    }

    #[test]
    fn two_hour_steady_ride_compresses_to_one_segment() {
        let raw_samples = vec![245; 2 * 60 * 60];
        let triplets = bucket_and_encode_power_segments(&raw_samples);
        assert_eq!(triplets.len(), 1);
        assert_eq!(triplets[0], [245, 245, 2 * 60 * 60]);
    }
}
