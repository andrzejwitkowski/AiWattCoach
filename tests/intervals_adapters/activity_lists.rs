use aiwattcoach::{
    adapters::intervals_icu::client::IntervalsIcuClient,
    domain::intervals::{DateRange, IntervalsApiPort},
};

use crate::support::{test_credentials, ResponseActivity, TestIntervalsServer};

#[tokio::test]
async fn intervals_client_lists_activities_and_normalizes_metrics() {
    let server = TestIntervalsServer::start().await;
    server.push_activity(ResponseActivity::sample("i101", "Tempo Ride"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activities = client
        .list_activities(
            &test_credentials(),
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].id, "i101");
    assert_eq!(activities[0].metrics.normalized_power_watts, Some(238));
    assert_eq!(activities[0].metrics.training_stress_score, Some(72));
    assert_eq!(activities[0].metrics.intensity_factor, Some(0.84));
    assert_eq!(activities[0].metrics.efficiency_factor, Some(1.28));

    let requests = server.requests();
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/athlete/athlete-7/activities");
}

#[tokio::test]
async fn intervals_client_accepts_numeric_zone_ids_in_activity_list_response() {
    let server = TestIntervalsServer::start().await;
    server.set_list_activities_raw(serde_json::json!([
        ResponseActivity::sample("i101", "Tempo Ride"),
        {
            "id": "bad-1",
            "start_date_local": "2025-01-13T08:00:00",
            "start_date": "2025-01-13T07:00:00Z",
            "type": "Ride",
            "name": "Broken Ride",
            "stream_types": null,
            "tags": null,
            "pace_zone_times": null,
            "gap_zone_times": null,
            "interval_summary": null,
            "skyline_chart_bytes": null,
            "icu_hr_zone_times": null,
            "icu_intervals": null,
            "icu_groups": null,
            "icu_zone_times": [
                { "id": 1, "secs": 120 }
            ]
        }
    ]));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activities = client
        .list_activities(
            &test_credentials(),
            &DateRange {
                oldest: "2025-01-01".to_string(),
                newest: "2025-01-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(activities.len(), 2);
    assert_eq!(activities[0].id, "i101");
    assert_eq!(activities[1].id, "bad-1");
    assert_eq!(activities[1].details.power_zone_times[0].zone_id, "1");
}

#[tokio::test]
async fn intervals_client_accepts_single_string_skyline_chart_bytes_in_activity_list_response() {
    let server = TestIntervalsServer::start().await;
    server.set_list_activities_raw(serde_json::json!([
        {
            "id": "i777",
            "icu_athlete_id": "athlete-7",
            "start_date_local": "2025-01-13T08:00:00",
            "start_date": "2025-01-13T07:00:00Z",
            "type": "Ride",
            "name": "Encoded Skyline Ride",
            "source": "WAHOO",
            "external_id": "ext-777",
            "device_name": "Bolt",
            "moving_time": 1800,
            "elapsed_time": 1805,
            "trainer": true,
            "commute": false,
            "race": false,
            "has_heartrate": false,
            "stream_types": ["time", "temp"],
            "tags": [],
            "pace_zone_times": null,
            "gap_zone_times": null,
            "interval_summary": [],
            "skyline_chart_bytes": "CAcSAtJFGgFAIgECKAE=",
            "icu_hr_zone_times": null,
            "icu_intervals": null,
            "icu_groups": null,
            "icu_zone_times": []
        }
    ]));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activities = client
        .list_activities(
            &test_credentials(),
            &DateRange {
                oldest: "2025-01-01".to_string(),
                newest: "2025-01-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].id, "i777");
    assert_eq!(
        activities[0].details.skyline_chart,
        vec!["CAcSAtJFGgFAIgECKAE="]
    );
    assert_eq!(activities[0].stream_types, vec!["temp"]);
}
