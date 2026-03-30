use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseEvent {
    id: i64,
    start_date_local: String,
    name: Option<String>,
    category: String,
    description: Option<String>,
    indoor: Option<bool>,
    color: Option<String>,
    workout_doc: Option<String>,
}

impl ResponseEvent {
    pub(crate) fn sample(id: i64, name: &str) -> Self {
        Self {
            id,
            start_date_local: "2026-03-22".to_string(),
            name: Some(name.to_string()),
            category: "WORKOUT".to_string(),
            description: Some("structured workout".to_string()),
            indoor: Some(true),
            color: Some("blue".to_string()),
            workout_doc: Some("- 5min 55%".to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivity {
    id: String,
    start_date_local: String,
    start_date: Option<String>,
    #[serde(rename = "type")]
    activity_type: Option<String>,
    name: Option<String>,
    description: Option<String>,
    source: Option<String>,
    external_id: Option<String>,
    device_name: Option<String>,
    icu_athlete_id: Option<String>,
    distance: Option<f64>,
    moving_time: Option<i32>,
    elapsed_time: Option<i32>,
    total_elevation_gain: Option<f64>,
    total_elevation_loss: Option<f64>,
    average_speed: Option<f64>,
    max_speed: Option<f64>,
    average_heartrate: Option<i32>,
    max_heartrate: Option<i32>,
    average_cadence: Option<f64>,
    pub(crate) trainer: Option<bool>,
    pub(crate) commute: Option<bool>,
    pub(crate) race: Option<bool>,
    has_heartrate: Option<bool>,
    pub(crate) stream_types: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    interval_summary: Option<Vec<String>>,
    skyline_chart_bytes: Option<Vec<String>>,
    icu_hr_zone_times: Option<Vec<i32>>,
    pace_zone_times: Option<Vec<i32>>,
    gap_zone_times: Option<Vec<i32>>,
    icu_training_load: Option<i32>,
    icu_weighted_avg_watts: Option<i32>,
    icu_intensity: Option<f64>,
    icu_efficiency_factor: Option<f64>,
    icu_variability_index: Option<f64>,
    icu_average_watts: Option<i32>,
    icu_ftp: Option<i32>,
    icu_joules: Option<i32>,
    calories: Option<i32>,
    trimp: Option<f64>,
    power_load: Option<i32>,
    hr_load: Option<i32>,
    pace_load: Option<i32>,
    strain_score: Option<f64>,
    icu_intervals: Option<Vec<ResponseActivityInterval>>,
    icu_groups: Option<Vec<ResponseActivityGroup>>,
}

impl ResponseActivity {
    pub(crate) fn sample(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            start_date_local: "2026-03-22T08:00:00".to_string(),
            start_date: Some("2026-03-22T07:00:00Z".to_string()),
            activity_type: Some("Ride".to_string()),
            name: Some(name.to_string()),
            description: Some("structured ride".to_string()),
            source: Some("UPLOAD".to_string()),
            external_id: Some(format!("external-{id}")),
            device_name: Some("Garmin".to_string()),
            icu_athlete_id: Some("athlete-7".to_string()),
            distance: Some(40200.0),
            moving_time: Some(3600),
            elapsed_time: Some(3700),
            total_elevation_gain: Some(420.0),
            total_elevation_loss: Some(415.0),
            average_speed: Some(11.2),
            max_speed: Some(16.0),
            average_heartrate: Some(148),
            max_heartrate: Some(174),
            average_cadence: Some(88.0),
            trainer: Some(false),
            commute: Some(false),
            race: Some(false),
            has_heartrate: Some(true),
            stream_types: Some(vec!["watts".to_string()]),
            tags: Some(vec!["tempo".to_string()]),
            interval_summary: Some(vec!["tempo".to_string()]),
            skyline_chart_bytes: Some(vec![]),
            icu_hr_zone_times: Some(vec![60, 120]),
            pace_zone_times: Some(vec![]),
            gap_zone_times: Some(vec![]),
            icu_training_load: Some(72),
            icu_weighted_avg_watts: Some(238),
            icu_intensity: Some(0.84),
            icu_efficiency_factor: Some(1.28),
            icu_variability_index: Some(1.04),
            icu_average_watts: Some(228),
            icu_ftp: Some(283),
            icu_joules: Some(820),
            calories: Some(690),
            trimp: Some(92.0),
            power_load: Some(72),
            hr_load: Some(66),
            pace_load: None,
            strain_score: Some(13.7),
            icu_intervals: Some(vec![]),
            icu_groups: Some(vec![]),
        }
    }

    pub(crate) fn sparse_strava_stub(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            start_date_local: "2026-03-22T08:00:00".to_string(),
            start_date: Some("2026-03-22T07:00:00Z".to_string()),
            activity_type: Some("Ride".to_string()),
            name: Some(name.to_string()),
            description: None,
            source: Some("STRAVA".to_string()),
            external_id: Some(format!("strava-{id}")),
            device_name: None,
            icu_athlete_id: Some("athlete-7".to_string()),
            distance: Some(40200.0),
            moving_time: Some(3600),
            elapsed_time: Some(3700),
            total_elevation_gain: Some(420.0),
            total_elevation_loss: Some(415.0),
            average_speed: Some(11.2),
            max_speed: Some(16.0),
            average_heartrate: Some(148),
            max_heartrate: Some(174),
            average_cadence: Some(88.0),
            trainer: Some(false),
            commute: Some(false),
            race: Some(false),
            has_heartrate: Some(true),
            stream_types: None,
            tags: Some(vec!["strava".to_string()]),
            interval_summary: None,
            skyline_chart_bytes: None,
            icu_hr_zone_times: None,
            pace_zone_times: None,
            gap_zone_times: None,
            icu_training_load: Some(72),
            icu_weighted_avg_watts: Some(238),
            icu_intensity: Some(0.84),
            icu_efficiency_factor: Some(1.28),
            icu_variability_index: Some(1.04),
            icu_average_watts: Some(228),
            icu_ftp: Some(283),
            icu_joules: Some(820),
            calories: Some(690),
            trimp: Some(92.0),
            power_load: Some(72),
            hr_load: Some(66),
            pace_load: None,
            strain_score: Some(13.7),
            icu_intervals: Some(vec![]),
            icu_groups: Some(vec![]),
        }
    }

    pub(crate) fn with_inline_intervals(mut self) -> Self {
        self.icu_intervals = Some(vec![ResponseActivityInterval::sample()]);
        self.icu_groups = Some(vec![ResponseActivityGroup::sample()]);
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivityIntervals {
    icu_intervals: Vec<ResponseActivityInterval>,
    icu_groups: Vec<ResponseActivityGroup>,
}

impl ResponseActivityIntervals {
    pub(crate) fn empty() -> Self {
        Self {
            icu_intervals: Vec::new(),
            icu_groups: Vec::new(),
        }
    }

    pub(crate) fn sample() -> Self {
        Self {
            icu_intervals: vec![ResponseActivityInterval::sample()],
            icu_groups: vec![ResponseActivityGroup::sample()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivityInterval {
    id: Option<i32>,
    label: Option<String>,
    #[serde(rename = "type")]
    interval_type: Option<String>,
    group_id: Option<String>,
    start_index: Option<i32>,
    end_index: Option<i32>,
    start_time: Option<i32>,
    end_time: Option<i32>,
    moving_time: Option<i32>,
    elapsed_time: Option<i32>,
    distance: Option<f64>,
    average_watts: Option<i32>,
    weighted_average_watts: Option<i32>,
    training_load: Option<f64>,
    average_heartrate: Option<i32>,
    average_cadence: Option<f64>,
    average_speed: Option<f64>,
    average_stride: Option<f64>,
    zone: Option<i32>,
}

impl ResponseActivityInterval {
    pub(crate) fn sample() -> Self {
        Self {
            id: Some(1),
            label: Some("Tempo".to_string()),
            interval_type: Some("WORK".to_string()),
            group_id: Some("g1".to_string()),
            start_index: Some(10),
            end_index: Some(50),
            start_time: Some(600),
            end_time: Some(1200),
            moving_time: Some(600),
            elapsed_time: Some(620),
            distance: Some(10000.0),
            average_watts: Some(250),
            weighted_average_watts: Some(260),
            training_load: Some(22.4),
            average_heartrate: Some(160),
            average_cadence: Some(90.0),
            average_speed: Some(11.5),
            average_stride: None,
            zone: Some(3),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivityGroup {
    id: String,
    count: Option<i32>,
    start_index: Option<i32>,
    moving_time: Option<i32>,
    elapsed_time: Option<i32>,
    distance: Option<f64>,
    average_watts: Option<i32>,
    weighted_average_watts: Option<i32>,
    training_load: Option<f64>,
    average_heartrate: Option<i32>,
    average_cadence: Option<f64>,
    average_speed: Option<f64>,
    average_stride: Option<f64>,
}

impl ResponseActivityGroup {
    pub(crate) fn sample() -> Self {
        Self {
            id: "g1".to_string(),
            count: Some(2),
            start_index: Some(10),
            moving_time: Some(1200),
            elapsed_time: Some(1240),
            distance: Some(20000.0),
            average_watts: Some(245),
            weighted_average_watts: Some(255),
            training_load: Some(44.0),
            average_heartrate: Some(158),
            average_cadence: Some(89.5),
            average_speed: Some(11.4),
            average_stride: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivityStream {
    #[serde(rename = "type")]
    stream_type: String,
    name: Option<String>,
    data: Option<serde_json::Value>,
    data2: Option<serde_json::Value>,
    #[serde(rename = "valueTypeIsArray")]
    value_type_is_array: bool,
    custom: bool,
    #[serde(rename = "allNull")]
    all_null: bool,
}

impl ResponseActivityStream {
    pub(crate) fn sample_time() -> Self {
        Self {
            stream_type: "time".to_string(),
            name: None,
            data: Some(serde_json::json!([0, 1, 2])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        }
    }

    pub(crate) fn sample_watts() -> Self {
        Self {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            data: Some(serde_json::json!([200, 210, 220])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseUpload {
    pub(crate) activities: Vec<ResponseActivityId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ResponseActivityId {
    pub(crate) id: String,
}
