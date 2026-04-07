use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};

use crate::domain::intervals::{Activity, ActivityDetails, ActivityMetrics};

use super::super::history::{build_daily_tss_map, build_load_trend};

#[test]
fn build_daily_tss_map_includes_zero_load_rest_days() {
    let start = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 4, 4).unwrap();
    let activities = vec![Activity {
        id: "ride-1".to_string(),
        athlete_id: None,
        start_date_local: "2026-04-03T08:00:00".to_string(),
        start_date: None,
        name: None,
        description: None,
        activity_type: None,
        source: None,
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
            training_stress_score: Some(80),
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
    }];

    let daily_tss = build_daily_tss_map(start, end, &activities);

    assert_eq!(daily_tss.len(), 4);
    assert_eq!(
        daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        Some(&0)
    );
    assert_eq!(
        daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 2).unwrap()),
        Some(&0)
    );
    assert_eq!(
        daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 3).unwrap()),
        Some(&80)
    );
    assert_eq!(
        daily_tss.get(&NaiveDate::from_ymd_opt(2026, 4, 4).unwrap()),
        Some(&0)
    );
}

#[test]
fn build_load_trend_uses_exponential_ctl_and_atl_not_simple_rolling_averages() {
    let start = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    let values = [100, 0, 100, 0, 100, 0, 100]
        .into_iter()
        .enumerate()
        .map(|(index, tss)| (start + Duration::days(index as i64), tss))
        .collect::<BTreeMap<_, _>>();

    let load_trend = build_load_trend(&values, 7, 42);
    let last = load_trend.last().unwrap();

    assert_eq!(last.rolling_tss_7d, Some(57.14));
    assert_eq!(last.ctl, Some(8.87));
    assert_eq!(last.atl, Some(38.16));
    assert_eq!(last.tsb, Some(-29.29));
}
