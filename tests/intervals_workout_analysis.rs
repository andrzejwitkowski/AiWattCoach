use aiwattcoach::domain::intervals::{
    find_best_activity_match, parse_workout_doc, Activity, ActivityDetails, ActivityInterval,
    ActivityMetrics, ActivityStream,
};
use serde_json::json;

#[test]
fn parse_workout_doc_expands_repeats_and_estimates_summary_metrics() {
    let parsed = parse_workout_doc(Some("- 10min 55%\n- 4x8min 95%\n- 5min 55%"), Some(300));

    assert_eq!(parsed.intervals.len(), 3);
    assert_eq!(parsed.intervals[0].definition, "- 10min 55%");
    assert_eq!(parsed.intervals[1].repeat_count, 4);
    assert_eq!(parsed.intervals[1].duration_seconds, Some(480));
    assert_eq!(parsed.intervals[1].min_target_percent_ftp, Some(95.0));
    assert_eq!(parsed.intervals[1].max_target_percent_ftp, Some(95.0));
    assert_eq!(parsed.intervals[1].target_percent_ftp, Some(95.0));
    assert_eq!(parsed.intervals[1].zone_id, Some(4));

    assert_eq!(parsed.segments.len(), 6);
    assert_eq!(parsed.segments[0].duration_seconds, 600);
    assert_eq!(parsed.segments[1].label, "4x8min 95% #1");
    assert_eq!(parsed.segments[4].label, "4x8min 95% #4");
    assert_eq!(parsed.segments[5].zone_id, Some(1));

    assert_eq!(parsed.summary.total_segments, 6);
    assert_eq!(parsed.summary.total_duration_seconds, 2820);
    assert_eq!(parsed.summary.estimated_normalized_power_watts, Some(262));
    assert_eq!(parsed.summary.estimated_average_power_watts, Some(247));
    assert_eq!(parsed.summary.estimated_intensity_factor, Some(0.874));
    assert_eq!(parsed.summary.estimated_training_stress_score, Some(59.8));
}

#[test]
fn find_best_activity_match_prefers_detected_intervals_and_extracts_streams() {
    let parsed = parse_workout_doc(Some("- 2x8min 95%"), Some(300));
    let activity = sample_activity();

    let matched =
        find_best_activity_match(&parsed, &[activity], Some(300)).expect("activity match");

    assert_eq!(matched.activity_id, "ride-1");
    assert_eq!(matched.matched_intervals.len(), 2);
    assert_eq!(matched.matched_intervals[0].planned_segment_order, 0);
    assert_eq!(
        matched.matched_intervals[0].actual_start_time_seconds,
        Some(600)
    );
    assert_eq!(
        matched.matched_intervals[1].actual_end_time_seconds,
        Some(1740)
    );
    assert_eq!(matched.power_values.len(), 1800);
    assert_eq!(matched.cadence_values.len(), 1800);
    assert_eq!(matched.heart_rate_values.len(), 1800);
    assert_eq!(matched.speed_values.len(), 1800);
    assert_eq!(matched.average_power_watts, Some(247));
    assert_eq!(matched.normalized_power_watts, Some(258));
    assert_eq!(matched.training_stress_score, Some(76));
    assert!(
        matched.compliance_score >= 0.9,
        "expected strong compliance score"
    );
}

#[test]
fn ignores_invalid_stream_samples_when_extracting_actual_workout_data() {
    let parsed = parse_workout_doc(Some("- 2x8min 95%"), Some(300));
    let mut activity = sample_activity();
    activity.details.streams = vec![
        ActivityStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            data: Some(json!([180, null, "bad", 220])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
        ActivityStream {
            stream_type: "heartrate".to_string(),
            name: Some("Heart Rate".to_string()),
            data: Some(json!([130, null, 150])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
    ];

    let matched =
        find_best_activity_match(&parsed, &[activity], Some(300)).expect("activity match");

    assert_eq!(matched.power_values, vec![180, 220]);
    assert_eq!(matched.heart_rate_values, vec![130, 150]);
}

#[test]
fn parse_workout_doc_supports_zone_tokens_for_segments_and_summary() {
    let parsed = parse_workout_doc(Some("10m Z2\n5m Z4\n2m Z6"), Some(300));

    assert_eq!(parsed.intervals.len(), 3);
    assert_eq!(parsed.intervals[0].min_target_percent_ftp, Some(65.0));
    assert_eq!(parsed.intervals[0].max_target_percent_ftp, Some(65.0));
    assert_eq!(parsed.intervals[0].target_percent_ftp, Some(65.0));
    assert_eq!(parsed.intervals[0].zone_id, Some(2));
    assert_eq!(parsed.intervals[1].target_percent_ftp, Some(98.0));
    assert_eq!(parsed.intervals[1].zone_id, Some(4));
    assert_eq!(parsed.intervals[2].target_percent_ftp, Some(130.0));
    assert_eq!(parsed.intervals[2].zone_id, Some(6));

    assert_eq!(parsed.segments.len(), 3);
    assert_eq!(parsed.segments[0].duration_seconds, 600);
    assert_eq!(parsed.segments[1].duration_seconds, 300);
    assert_eq!(parsed.segments[2].duration_seconds, 120);
    assert_eq!(parsed.summary.total_duration_seconds, 1020);
    assert_eq!(parsed.summary.estimated_intensity_factor, Some(0.919));
    assert_eq!(parsed.summary.estimated_average_power_watts, Some(247));
    assert_eq!(parsed.summary.estimated_normalized_power_watts, Some(276));
    assert_eq!(parsed.summary.estimated_training_stress_score, Some(23.9));
}

#[test]
fn parse_workout_doc_preserves_percent_ranges() {
    let parsed = parse_workout_doc(Some("- 3x10min 88-92%"), Some(300));

    assert_eq!(parsed.intervals.len(), 1);
    assert_eq!(parsed.intervals[0].min_target_percent_ftp, Some(88.0));
    assert_eq!(parsed.intervals[0].max_target_percent_ftp, Some(92.0));
    assert_eq!(parsed.intervals[0].target_percent_ftp, Some(90.0));
    assert_eq!(parsed.segments.len(), 3);
    assert_eq!(parsed.segments[0].min_target_percent_ftp, Some(88.0));
    assert_eq!(parsed.segments[0].max_target_percent_ftp, Some(92.0));
    assert_eq!(parsed.summary.total_duration_seconds, 1800);
}

#[test]
fn parse_workout_doc_caps_segment_expansion_for_large_repeat_counts() {
    let parsed = parse_workout_doc(Some("20000x1s"), None);

    assert_eq!(parsed.segments.len(), 10_000);
    assert_eq!(parsed.summary.total_segments, 10_000);
    assert_eq!(parsed.summary.total_duration_seconds, 10_000);
}

#[test]
fn parse_workout_doc_stops_before_offset_overflow() {
    let parsed = parse_workout_doc(Some("2x2147483647s"), None);

    assert_eq!(parsed.segments.len(), 1);
    assert_eq!(parsed.segments[0].start_offset_seconds, 0);
    assert_eq!(parsed.segments[0].end_offset_seconds, i32::MAX);
}

fn sample_activity() -> Activity {
    Activity {
        id: "ride-1".to_string(),
        athlete_id: Some("athlete-42".to_string()),
        start_date_local: "2026-03-22T08:00:00".to_string(),
        start_date: Some("2026-03-22T07:00:00Z".to_string()),
        name: Some("Sweet Spot Session".to_string()),
        description: Some("executed outdoors".to_string()),
        activity_type: Some("Ride".to_string()),
        source: Some("GARMIN".to_string()),
        external_id: Some("external-ride-1".to_string()),
        device_name: Some("Garmin Edge".to_string()),
        distance_meters: Some(35200.0),
        moving_time_seconds: Some(1800),
        elapsed_time_seconds: Some(1860),
        total_elevation_gain_meters: Some(280.0),
        total_elevation_loss_meters: Some(275.0),
        average_speed_mps: Some(9.6),
        max_speed_mps: Some(15.3),
        average_heart_rate_bpm: Some(154),
        max_heart_rate_bpm: Some(176),
        average_cadence_rpm: Some(87.0),
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: true,
        stream_types: vec![
            "watts".to_string(),
            "cadence".to_string(),
            "heartrate".to_string(),
            "velocity_smooth".to_string(),
        ],
        tags: vec!["workout".to_string()],
        metrics: ActivityMetrics {
            training_stress_score: Some(76),
            normalized_power_watts: Some(258),
            intensity_factor: Some(0.86),
            efficiency_factor: Some(1.2),
            variability_index: Some(1.04),
            average_power_watts: Some(247),
            ftp_watts: Some(300),
            total_work_joules: Some(760),
            calories: Some(640),
            trimp: Some(83.0),
            power_load: Some(76),
            heart_rate_load: Some(71),
            pace_load: None,
            strain_score: Some(14.1),
        },
        details: ActivityDetails {
            intervals: vec![
                ActivityInterval {
                    id: Some(1),
                    label: Some("Work 1".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(600),
                    end_index: Some(1080),
                    start_time_seconds: Some(600),
                    end_time_seconds: Some(1080),
                    moving_time_seconds: Some(480),
                    elapsed_time_seconds: Some(480),
                    distance_meters: Some(8200.0),
                    average_power_watts: Some(284),
                    normalized_power_watts: Some(287),
                    training_stress_score: Some(18.0),
                    average_heart_rate_bpm: Some(161),
                    average_cadence_rpm: Some(89.0),
                    average_speed_mps: Some(10.2),
                    average_stride_meters: None,
                    zone: Some(4),
                },
                ActivityInterval {
                    id: Some(2),
                    label: Some("Work 2".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(1260),
                    end_index: Some(1740),
                    start_time_seconds: Some(1260),
                    end_time_seconds: Some(1740),
                    moving_time_seconds: Some(480),
                    elapsed_time_seconds: Some(480),
                    distance_meters: Some(8300.0),
                    average_power_watts: Some(286),
                    normalized_power_watts: Some(289),
                    training_stress_score: Some(18.2),
                    average_heart_rate_bpm: Some(164),
                    average_cadence_rpm: Some(88.0),
                    average_speed_mps: Some(10.4),
                    average_stride_meters: None,
                    zone: Some(4),
                },
            ],
            interval_groups: Vec::new(),
            streams: vec![
                ActivityStream {
                    stream_type: "watts".to_string(),
                    name: Some("Power".to_string()),
                    data: Some(json!(repeat_i32(
                        1800,
                        180,
                        [(600, 1080, 284), (1260, 1740, 286)],
                    ))),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                ActivityStream {
                    stream_type: "cadence".to_string(),
                    name: Some("Cadence".to_string()),
                    data: Some(json!(repeat_i32(
                        1800,
                        84,
                        [(600, 1080, 89), (1260, 1740, 88)],
                    ))),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                ActivityStream {
                    stream_type: "heartrate".to_string(),
                    name: Some("Heart Rate".to_string()),
                    data: Some(json!(repeat_i32(
                        1800,
                        138,
                        [(600, 1080, 161), (1260, 1740, 164)],
                    ))),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                ActivityStream {
                    stream_type: "velocity_smooth".to_string(),
                    name: Some("Speed".to_string()),
                    data: Some(json!(repeat_f64(
                        1800,
                        8.7,
                        [(600, 1080, 10.2), (1260, 1740, 10.4)],
                    ))),
                    data2: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
            ],
            interval_summary: vec!["2x8min sweet spot".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}

fn repeat_i32(len: usize, default: i32, ranges: [(usize, usize, i32); 2]) -> Vec<i32> {
    let mut values = vec![default; len];
    for (start, end, range_value) in ranges {
        for value in values.iter_mut().take(end).skip(start) {
            *value = range_value;
        }
    }
    values
}

fn repeat_f64(len: usize, default: f64, ranges: [(usize, usize, f64); 2]) -> Vec<f64> {
    let mut values = vec![default; len];
    for (start, end, range_value) in ranges {
        for value in values.iter_mut().take(end).skip(start) {
            *value = range_value;
        }
    }
    values
}
