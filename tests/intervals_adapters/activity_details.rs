use aiwattcoach::{
    adapters::intervals_icu::client::IntervalsIcuClient, domain::intervals::IntervalsApiPort,
};
use axum::http::StatusCode;

use crate::support::{
    test_credentials, ResponseActivity, ResponseActivityIntervals, ResponseActivityStream,
    TestIntervalsServer,
};

#[tokio::test]
async fn intervals_client_gets_activity_with_intervals_and_streams() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sample("i202", "Loaded Ride"));
    server.set_activity_intervals(ResponseActivityIntervals::sample());
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i202")
        .await
        .unwrap();

    assert_eq!(activity.id, "i202");
    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.streams.len(), 1);
    assert_eq!(activity.details.streams[0].stream_type, "watts");

    let requests = server.requests();
    assert_eq!(requests[0].path, "/api/v1/activity/i202");
    assert_eq!(requests[0].query, None);
    assert_eq!(requests[1].path, "/api/v1/activity/i202/intervals");
    assert_eq!(requests[1].query, None);
    assert_eq!(requests[2].path, "/api/v1/activity/i202/streams");
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| query.contains("types=watts")));
    assert_eq!(
        requests[2]
            .query
            .as_deref()
            .map(|query| query.matches("types=").count()),
        Some(1)
    );
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| query.contains("includeDefaults=true")));
}

#[tokio::test]
async fn intervals_client_ignores_time_streams_when_fetching_activity_details() {
    let server = TestIntervalsServer::start().await;
    let mut activity = ResponseActivity::sample("i206", "Loaded Ride");
    activity.stream_types = Some(vec!["time".to_string(), "watts".to_string()]);
    server.set_activity(activity);
    server.set_activity_intervals(ResponseActivityIntervals::sample());
    server.set_streams(vec![
        ResponseActivityStream::sample_time(),
        ResponseActivityStream::sample_watts(),
    ]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i206")
        .await
        .unwrap();

    assert_eq!(activity.stream_types, vec!["watts"]);
    assert_eq!(activity.details.streams.len(), 1);
    assert_eq!(activity.details.streams[0].stream_type, "watts");

    let requests = server.requests();
    assert_eq!(requests[2].path, "/api/v1/activity/i206/streams");
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| query.contains("types=watts")));
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| !query.contains("types=time")));
}

#[tokio::test]
async fn intervals_client_does_not_leak_base_intervals_when_dedicated_interval_fetch_fails() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sample("i250", "Loaded Ride"));
    server.set_activity_intervals_status(StatusCode::TOO_MANY_REQUESTS);
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i250")
        .await
        .unwrap();

    assert!(activity.details.intervals.is_empty());
    assert!(activity.details.interval_groups.is_empty());
    assert_eq!(activity.details.streams.len(), 1);
}

#[tokio::test]
async fn completed_activity_detail_enrichment_merges_sparse_base_with_intervals_and_streams() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub("i303", "Morning Ride"));
    server.set_activity_intervals(ResponseActivityIntervals::sample());
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i303")
        .await
        .unwrap();

    assert_eq!(activity.id, "i303");
    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.interval_groups.len(), 1);
    assert_eq!(activity.details.streams.len(), 1);
    assert_eq!(activity.stream_types, Vec::<String>::new());

    let requests = server.requests();
    assert_eq!(requests[0].path, "/api/v1/activity/i303");
    assert_eq!(requests[0].query, None);
    assert_eq!(requests[1].path, "/api/v1/activity/i303/intervals");
    assert_eq!(requests[1].query, None);
    assert_eq!(requests[2].path, "/api/v1/activity/i303/streams");
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| query.contains("includeDefaults=true")));
    assert!(requests[2]
        .query
        .as_deref()
        .is_some_and(|query| !query.contains("types=")));
}

#[tokio::test]
async fn completed_activity_detail_enrichment_falls_back_to_base_activity_with_intervals_when_dedicated_intervals_returns_422(
) {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub("i304", "Morning Ride"));
    server.set_activity_with_intervals(
        ResponseActivity::sample("i304", "Morning Ride").with_inline_intervals(),
    );
    server.set_activity_intervals_status(StatusCode::UNPROCESSABLE_ENTITY);
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i304")
        .await
        .unwrap();

    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.interval_groups.len(), 1);
    assert_eq!(activity.details.streams.len(), 1);

    let requests = server.requests();
    assert_eq!(requests[0].path, "/api/v1/activity/i304");
    assert_eq!(requests[0].query, None);
    assert_eq!(requests[1].path, "/api/v1/activity/i304/intervals");
    assert_eq!(requests[2].path, "/api/v1/activity/i304");
    assert_eq!(requests[2].query.as_deref(), Some("intervals=true"));
    assert_eq!(requests[3].path, "/api/v1/activity/i304/streams");
}

#[tokio::test]
async fn completed_activity_partial_enrichment_returns_base_activity_when_intervals_fail() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub("i202", "Loaded Ride"));
    server.set_activity_intervals_status(StatusCode::TOO_MANY_REQUESTS);
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client.get_activity(&test_credentials(), "i202").await;

    let activity = activity.expect("base activity fetch should fail open when intervals fail");

    assert_eq!(activity.id, "i202");
    assert!(activity.details.intervals.is_empty());
    assert!(activity.details.interval_groups.is_empty());
    assert_eq!(activity.details.streams.len(), 1);
}

#[tokio::test]
async fn completed_activity_partial_enrichment_returns_base_activity_when_streams_fail() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub("i203", "Loaded Ride"));
    server.set_activity_intervals(ResponseActivityIntervals::sample());
    server.set_streams_status(StatusCode::TOO_MANY_REQUESTS);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client.get_activity(&test_credentials(), "i203").await;

    let activity = activity.expect("base activity fetch should fail open when streams fail");

    assert_eq!(activity.id, "i203");
    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.interval_groups.len(), 1);
    assert!(activity.details.streams.is_empty());
}

#[tokio::test]
async fn completed_activity_detail_marks_strava_stub_when_all_enrichment_paths_are_unavailable() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub(
        "i206",
        "Unavailable Ride",
    ));
    server.set_activity_intervals_status(StatusCode::UNPROCESSABLE_ENTITY);
    server.set_streams_status(StatusCode::UNPROCESSABLE_ENTITY);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i206")
        .await
        .unwrap();

    assert!(activity.details.intervals.is_empty());
    assert!(activity.details.interval_groups.is_empty());
    assert!(activity.details.streams.is_empty());
    assert_eq!(
        activity.details_unavailable_reason.as_deref(),
        Some("Intervals.icu did not provide detailed data for this imported activity.")
    );
}

#[tokio::test]
async fn completed_activity_detail_does_not_mark_strava_stub_when_streams_fail_transiently() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub(
        "i207",
        "Transient Failure Ride",
    ));
    server.set_activity_intervals_status(StatusCode::UNPROCESSABLE_ENTITY);
    server.set_streams_status(StatusCode::TOO_MANY_REQUESTS);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i207")
        .await
        .unwrap();

    assert!(activity.details.intervals.is_empty());
    assert!(activity.details.interval_groups.is_empty());
    assert!(activity.details.streams.is_empty());
    assert_eq!(activity.details_unavailable_reason, None);
}

#[tokio::test]
async fn completed_activity_partial_enrichment_preserves_inline_intervals_when_dedicated_intervals_payload_is_malformed(
) {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sample("i204", "Loaded Ride").with_inline_intervals());
    server.set_activity_intervals_raw(serde_json::json!({ "icu_intervals": "bad-payload" }));
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i204")
        .await
        .unwrap();

    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.interval_groups.len(), 1);
    assert_eq!(activity.details.streams.len(), 1);
}

#[tokio::test]
async fn completed_activity_partial_enrichment_preserves_streams_when_streams_payload_is_malformed()
{
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sparse_strava_stub("i205", "Loaded Ride"));
    server.set_activity_intervals(ResponseActivityIntervals::sample());
    server.set_streams_raw(serde_json::json!({ "type": "watts" }));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(&test_credentials(), "i205")
        .await
        .unwrap();

    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.interval_groups.len(), 1);
    assert!(activity.details.streams.is_empty());
}
