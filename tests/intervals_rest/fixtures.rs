use aiwattcoach::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream,
    },
    intervals::{Activity, ActivityDetails, ActivityMetrics, ActivityStream, Event, EventCategory},
};
use axum::{
    body::to_bytes,
    http::{header, HeaderValue},
    response::Response,
};

use crate::app::RESPONSE_LIMIT_BYTES;

pub(crate) fn sample_event(id: i64, name: &str, workout_doc: Option<String>) -> Event {
    Event {
        id,
        start_date_local: "2026-03-22".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some(name.to_string()),
        category: EventCategory::Workout,
        description: Some("structured workout".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc,
    }
}

pub(crate) fn sample_activity(id: &str, name: &str) -> Activity {
    Activity {
        id: id.to_string(),
        athlete_id: Some("athlete-42".to_string()),
        start_date_local: "2026-03-22T08:00:00".to_string(),
        start_date: Some("2026-03-22T07:00:00Z".to_string()),
        name: Some(name.to_string()),
        description: Some("structured ride".to_string()),
        activity_type: Some("Ride".to_string()),
        source: Some("UPLOAD".to_string()),
        external_id: Some(format!("external-{id}")),
        device_name: Some("Garmin Edge".to_string()),
        distance_meters: Some(40200.0),
        moving_time_seconds: Some(3600),
        elapsed_time_seconds: Some(3700),
        total_elevation_gain_meters: Some(420.0),
        total_elevation_loss_meters: Some(415.0),
        average_speed_mps: Some(11.2),
        max_speed_mps: Some(16.0),
        average_heart_rate_bpm: Some(148),
        max_heart_rate_bpm: Some(174),
        average_cadence_rpm: Some(88.0),
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: true,
        stream_types: vec!["watts".to_string()],
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
            intervals: vec![aiwattcoach::domain::intervals::ActivityInterval {
                id: Some(1),
                label: Some("Tempo".to_string()),
                interval_type: Some("WORK".to_string()),
                group_id: Some("g1".to_string()),
                start_index: Some(10),
                end_index: Some(50),
                start_time_seconds: Some(600),
                end_time_seconds: Some(1200),
                moving_time_seconds: Some(600),
                elapsed_time_seconds: Some(620),
                distance_meters: Some(10000.0),
                average_power_watts: Some(250),
                normalized_power_watts: Some(260),
                training_stress_score: Some(22.4),
                average_heart_rate_bpm: Some(160),
                average_cadence_rpm: Some(90.0),
                average_speed_mps: Some(11.5),
                average_stride_meters: None,
                zone: Some(3),
            }],
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: vec![60, 120],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}

pub(crate) fn sample_completed_workout(
    id: &str,
    planned_workout_id: Option<String>,
) -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: id.to_string(),
        user_id: "user-1".to_string(),
        start_date_local: "2026-03-22T08:00:00".to_string(),
        source_activity_id: Some(id.to_string()),
        planned_workout_id,
        name: Some("VO2 Session Completed".to_string()),
        description: Some("Strong finish".to_string()),
        activity_type: Some("Ride".to_string()),
        external_id: Some(format!("external-{id}")),
        trainer: false,
        duration_seconds: Some(3600),
        distance_meters: Some(40200.0),
        metrics: CompletedWorkoutMetrics {
            training_stress_score: Some(72),
            normalized_power_watts: Some(238),
            intensity_factor: Some(0.84),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(228),
            ftp_watts: Some(283),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: vec![60, 120],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}

pub(crate) fn session_cookie(value: &str) -> HeaderValue {
    header::HeaderValue::from_str(&format!("aiwattcoach_session={value}; Path=/")).unwrap()
}

pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(response: Response) -> T {
    let parts = response.into_parts();
    let body = to_bytes(parts.1, RESPONSE_LIMIT_BYTES)
        .await
        .expect("body to be collected");
    serde_json::from_slice(&body).expect("valid JSON")
}

pub(crate) fn watts_stream(values: &[i64]) -> ActivityStream {
    ActivityStream {
        stream_type: "watts".to_string(),
        name: Some("Power".to_string()),
        data: Some(serde_json::json!(values)),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }
}
