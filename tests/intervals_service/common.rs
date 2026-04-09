use std::{future::Future, pin::Pin};

use aiwattcoach::domain::intervals::{
    Activity, ActivityDetails, ActivityMetrics, Event, EventCategory, IntervalsCredentials,
};

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub(crate) fn valid_credentials() -> IntervalsCredentials {
    IntervalsCredentials {
        api_key: "api-key-123".to_string(),
        athlete_id: "athlete-42".to_string(),
    }
}

pub(crate) fn sample_event(id: i64, name: &str) -> Event {
    Event {
        id,
        start_date_local: "2026-03-22".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some(name.to_string()),
        category: EventCategory::Workout,
        description: Some("structured workout".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc: Some("- 5min 55%".to_string()),
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
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}
