use mongodb::bson::{from_document, to_document};

use super::{
    merge_activity_for_storage, normalize_activity, normalize_activity_document, ActivityDocument,
};
use crate::domain::intervals::{
    Activity, ActivityDeduplicationIdentity, ActivityDetails, ActivityInterval,
    ActivityIntervalGroup, ActivityMetrics, ActivityStream,
};

#[test]
fn activity_document_bson_round_trip_preserves_enriched_completed_fields() {
    let payload = enriched_activity();
    let document = ActivityDocument {
        user_id: "user-1".to_string(),
        activity_id: payload.id.clone(),
        start_date_local: payload.start_date_local.clone(),
        event_id_hint: None,
        external_id_normalized: Some("external-i78".to_string()),
        fallback_identity_v1: Some("v1:2026-03-22T08:00|ride|3720|40200|false".to_string()),
        payload: payload.clone(),
    };

    let bson = to_document(&document).expect("serialize activity document");
    let restored: ActivityDocument = from_document(bson).expect("deserialize activity document");

    assert_eq!(restored.user_id, document.user_id);
    assert_eq!(restored.activity_id, document.activity_id);
    assert_eq!(restored.start_date_local, document.start_date_local);
    assert_eq!(
        restored.external_id_normalized,
        document.external_id_normalized
    );
    assert_eq!(restored.fallback_identity_v1, document.fallback_identity_v1);
    assert_eq!(restored.payload.metrics, payload.metrics);
    assert_eq!(
        restored.payload.details.intervals,
        payload.details.intervals
    );
    assert_eq!(
        restored.payload.details.interval_groups,
        payload.details.interval_groups
    );
    assert_eq!(restored.payload.details.streams, payload.details.streams);
    assert_eq!(restored.payload, payload);
}

#[test]
fn merge_sparse_activity_payload_preserves_existing_enriched_fields() {
    let existing = enriched_activity();
    let incoming = sparse_activity_stub(&existing.id);

    let merged = merge_activity_for_storage(Some(existing.clone()), incoming);

    assert_eq!(merged.id, existing.id);
    assert_eq!(merged.start_date_local, existing.start_date_local);
    assert_eq!(merged.name, existing.name);
    assert_eq!(merged.activity_type, existing.activity_type);
    assert_eq!(merged.distance_meters, existing.distance_meters);
    assert_eq!(merged.moving_time_seconds, existing.moving_time_seconds);
    assert_eq!(merged.metrics, existing.metrics);
    assert_eq!(merged.details, existing.details);
    assert_eq!(merged.stream_types, existing.stream_types);
    assert_eq!(merged.tags, existing.tags);
    assert!(merged.has_heart_rate);
}

#[test]
fn merge_richer_incoming_activity_replaces_existing_payload() {
    let existing = enriched_activity();
    let mut incoming = enriched_activity();
    incoming.name = Some("Updated Completed Workout".to_string());
    incoming.details.streams = vec![ActivityStream {
        stream_type: "heartrate".to_string(),
        name: Some("Heart Rate".to_string()),
        data: Some(serde_json::json!([140, 150, 160])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];

    let merged = merge_activity_for_storage(Some(existing), incoming.clone());

    assert_eq!(merged, incoming);
}

#[test]
fn merge_equal_richness_payloads_preserves_complementary_detail_buckets() {
    let mut existing = sparse_activity_stub("i79");
    existing.details.streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: Some("Power".to_string()),
        data: Some(serde_json::json!([150, 220, 280])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];
    existing.details.interval_groups = vec![ActivityIntervalGroup {
        id: "group-1".to_string(),
        count: Some(2),
        start_index: Some(0),
        moving_time_seconds: Some(600),
        elapsed_time_seconds: Some(600),
        distance_meters: None,
        average_power_watts: Some(240),
        normalized_power_watts: None,
        training_stress_score: None,
        average_heart_rate_bpm: None,
        average_cadence_rpm: None,
        average_speed_mps: None,
        average_stride_meters: None,
    }];

    let mut incoming = sparse_activity_stub("i79");
    incoming.details.intervals = vec![ActivityInterval {
        id: Some(7),
        label: Some("Work".to_string()),
        interval_type: Some("WORK".to_string()),
        group_id: Some("group-1".to_string()),
        start_index: Some(1),
        end_index: Some(10),
        start_time_seconds: Some(60),
        end_time_seconds: Some(300),
        moving_time_seconds: Some(240),
        elapsed_time_seconds: Some(240),
        distance_meters: None,
        average_power_watts: Some(260),
        normalized_power_watts: None,
        training_stress_score: None,
        average_heart_rate_bpm: None,
        average_cadence_rpm: None,
        average_speed_mps: None,
        average_stride_meters: None,
        zone: Some(4),
    }];
    incoming.details.interval_summary = vec!["work".to_string()];

    let merged = merge_activity_for_storage(Some(existing.clone()), incoming.clone());

    assert_eq!(merged.details.intervals, incoming.details.intervals);
    assert_eq!(
        merged.details.interval_summary,
        incoming.details.interval_summary
    );
    assert_eq!(merged.details.streams, existing.details.streams);
    assert_eq!(
        merged.details.interval_groups,
        existing.details.interval_groups
    );
}

#[test]
fn merge_activity_for_storage_drops_time_streams() {
    let mut incoming = enriched_activity();
    incoming.stream_types = vec!["time".to_string(), "watts".to_string()];
    incoming.details.streams = vec![
        ActivityStream {
            stream_type: "time".to_string(),
            name: None,
            data: Some(serde_json::json!([0, 1, 2])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
        ActivityStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            data: Some(serde_json::json!([120, 250, 310])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
    ];

    let merged = merge_activity_for_storage(None, incoming);

    assert_eq!(merged.stream_types, vec!["watts".to_string()]);
    assert_eq!(merged.details.streams.len(), 1);
    assert_eq!(merged.details.streams[0].stream_type, "watts");
}

#[test]
fn normalize_activity_document_drops_time_streams_and_refreshes_dedupe_fields() {
    let mut payload = enriched_activity();
    payload.external_id = Some("  EXTERNAL-I78  ".to_string());
    payload.stream_types = vec!["time".to_string(), "watts".to_string()];
    payload.details.streams = vec![
        ActivityStream {
            stream_type: "time".to_string(),
            name: None,
            data: Some(serde_json::json!([0, 1, 2])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
        ActivityStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            data: Some(serde_json::json!([120, 250, 310])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        },
    ];
    let normalized_payload = normalize_activity(payload.clone());
    let expected_dedupe_identity =
        ActivityDeduplicationIdentity::from_activity(&normalized_payload);

    let normalized_document = normalize_activity_document(ActivityDocument {
        user_id: "user-1".to_string(),
        activity_id: payload.id.clone(),
        start_date_local: payload.start_date_local.clone(),
        event_id_hint: None,
        external_id_normalized: None,
        fallback_identity_v1: None,
        payload,
    });

    assert_eq!(normalized_document.payload.stream_types, vec!["watts"]);
    assert_eq!(normalized_document.payload.details.streams.len(), 1);
    assert_eq!(
        normalized_document.external_id_normalized,
        expected_dedupe_identity.normalized_external_id
    );
    assert_eq!(
        normalized_document.fallback_identity_v1,
        expected_dedupe_identity.fallback_identity
    );
}

#[test]
fn normalize_activity_document_keeps_clean_document_unchanged() {
    let payload = enriched_activity();
    let dedupe_identity = ActivityDeduplicationIdentity::from_activity(&payload);
    let document = ActivityDocument {
        user_id: "user-1".to_string(),
        activity_id: payload.id.clone(),
        start_date_local: payload.start_date_local.clone(),
        event_id_hint: None,
        external_id_normalized: dedupe_identity.normalized_external_id,
        fallback_identity_v1: dedupe_identity.fallback_identity,
        payload,
    };

    let normalized_document = normalize_activity_document(document.clone());

    assert_eq!(normalized_document, document);
}

fn sparse_activity_stub(id: &str) -> Activity {
    Activity {
        id: id.to_string(),
        athlete_id: None,
        start_date_local: "2026-03-22T08:00:00".to_string(),
        start_date: None,
        name: None,
        description: None,
        activity_type: None,
        source: Some("STRAVA".to_string()),
        external_id: None,
        device_name: None,
        distance_meters: None,
        moving_time_seconds: None,
        elapsed_time_seconds: None,
        total_elevation_gain_meters: None,
        total_elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: false,
        stream_types: Vec::new(),
        tags: Vec::new(),
        metrics: ActivityMetrics {
            training_stress_score: None,
            normalized_power_watts: None,
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: None,
            ftp_watts: None,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: ActivityDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}

fn enriched_activity() -> Activity {
    Activity {
        id: "i78".to_string(),
        athlete_id: Some("athlete-42".to_string()),
        start_date_local: "2026-03-22T08:00:00".to_string(),
        start_date: Some("2026-03-22T07:00:00Z".to_string()),
        name: Some("Completed Workout".to_string()),
        description: Some("structured ride".to_string()),
        activity_type: Some("Ride".to_string()),
        source: Some("STRAVA".to_string()),
        external_id: Some("external-i78".to_string()),
        device_name: Some("Garmin Edge".to_string()),
        distance_meters: Some(40200.0),
        moving_time_seconds: Some(3600),
        elapsed_time_seconds: Some(3720),
        total_elevation_gain_meters: Some(510.0),
        total_elevation_loss_meters: Some(505.0),
        average_speed_mps: Some(11.1),
        max_speed_mps: Some(16.4),
        average_heart_rate_bpm: Some(148),
        max_heart_rate_bpm: Some(175),
        average_cadence_rpm: Some(89.5),
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: true,
        stream_types: vec!["watts".to_string(), "heartrate".to_string()],
        tags: vec!["tempo".to_string()],
        metrics: ActivityMetrics {
            training_stress_score: Some(72),
            normalized_power_watts: Some(238),
            intensity_factor: Some(0.84),
            efficiency_factor: Some(1.28),
            variability_index: Some(1.04),
            average_power_watts: Some(228),
            ftp_watts: Some(283),
            total_work_joules: Some(820),
            calories: Some(690),
            trimp: Some(92.0),
            power_load: Some(72),
            heart_rate_load: Some(66),
            pace_load: None,
            strain_score: Some(13.7),
        },
        details: ActivityDetails {
            intervals: vec![ActivityInterval {
                id: Some(1),
                label: Some("Threshold".to_string()),
                interval_type: Some("WORK".to_string()),
                group_id: Some("set-1".to_string()),
                start_index: Some(10),
                end_index: Some(20),
                start_time_seconds: Some(300),
                end_time_seconds: Some(600),
                moving_time_seconds: Some(300),
                elapsed_time_seconds: Some(300),
                distance_meters: Some(2500.0),
                average_power_watts: Some(285),
                normalized_power_watts: Some(290),
                training_stress_score: Some(18.5),
                average_heart_rate_bpm: Some(168),
                average_cadence_rpm: Some(94.0),
                average_speed_mps: Some(8.2),
                average_stride_meters: None,
                zone: Some(4),
            }],
            interval_groups: vec![ActivityIntervalGroup {
                id: "set-1".to_string(),
                count: Some(3),
                start_index: Some(10),
                moving_time_seconds: Some(900),
                elapsed_time_seconds: Some(900),
                distance_meters: Some(7500.0),
                average_power_watts: Some(280),
                normalized_power_watts: Some(286),
                training_stress_score: Some(55.5),
                average_heart_rate_bpm: Some(165),
                average_cadence_rpm: Some(92.0),
                average_speed_mps: Some(8.0),
                average_stride_meters: None,
            }],
            streams: vec![ActivityStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                data: Some(serde_json::json!([120, 250, 310])),
                data2: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: vec!["z2".to_string(), "z4".to_string()],
            power_zone_times: Vec::new(),
            heart_rate_zone_times: vec![120, 240],
            pace_zone_times: vec![60],
            gap_zone_times: vec![90],
        },
        details_unavailable_reason: None,
    }
}
