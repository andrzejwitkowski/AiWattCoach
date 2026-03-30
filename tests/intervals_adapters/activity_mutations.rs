use aiwattcoach::{
    adapters::intervals_icu::client::IntervalsIcuClient,
    domain::intervals::{IntervalsApiPort, UpdateActivity, UploadActivity},
};
use axum::http::StatusCode;

use crate::support::{
    test_credentials, ResponseActivity, ResponseActivityStream, TestIntervalsServer,
};

#[tokio::test]
async fn intervals_client_uploads_activity_and_fetches_uploaded_details() {
    let server = TestIntervalsServer::start().await;
    server.set_upload_ids(vec!["i303".to_string()]);
    server.set_activity(ResponseActivity::sample("i303", "Uploaded Ride"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();

    let result = client
        .upload_activity(
            &credentials,
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![0, 159, 146, 150],
                name: Some("Uploaded Ride".to_string()),
                description: Some("desc".to_string()),
                device_name: Some("Garmin".to_string()),
                external_id: Some("ext-303".to_string()),
                paired_event_id: Some(9),
            },
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.activity_ids, vec!["i303".to_string()]);
    assert_eq!(result.activities[0].id, "i303");

    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/api/v1/athlete/athlete-7/activities");
    assert!(requests[0]
        .query
        .as_deref()
        .unwrap_or_default()
        .contains("paired_event_id=9"));
    assert!(requests[0]
        .body
        .as_ref()
        .is_some_and(|body| body.windows(4).any(|window| window == [0, 159, 146, 150])));
    assert_eq!(requests[1].path, "/api/v1/activity/i303");
    assert_eq!(requests[2].path, "/api/v1/activity/i303/intervals");
    assert_eq!(requests[3].path, "/api/v1/activity/i303/streams");
}

#[tokio::test]
async fn upload_activity_returns_base_activity_when_dedicated_intervals_fail() {
    let server = TestIntervalsServer::start().await;
    server.set_upload_ids(vec!["i350".to_string()]);
    server.set_activity(ResponseActivity::sample("i350", "Uploaded Ride").with_inline_intervals());
    server.set_activity_intervals_status(StatusCode::TOO_MANY_REQUESTS);
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();

    let result = client
        .upload_activity(
            &credentials,
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Uploaded Ride".to_string()),
                description: None,
                device_name: None,
                external_id: None,
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.activities.len(), 1);
    assert_eq!(result.activities[0].details.intervals.len(), 1);
    assert_eq!(result.activities[0].details.interval_groups.len(), 1);
    assert_eq!(result.activities[0].details.streams.len(), 1);
}

#[tokio::test]
async fn update_activity_returns_base_activity_when_dedicated_intervals_fail() {
    let server = TestIntervalsServer::start().await;
    server.set_updated_activity(
        ResponseActivity::sample("i450", "Updated Ride").with_inline_intervals(),
    );
    server.set_activity(ResponseActivity::sample("i450", "Updated Ride").with_inline_intervals());
    server.set_activity_intervals_status(StatusCode::TOO_MANY_REQUESTS);
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();

    let updated = client
        .update_activity(
            &credentials,
            "i450",
            UpdateActivity {
                name: Some("Updated Ride".to_string()),
                description: None,
                activity_type: Some("Ride".to_string()),
                trainer: Some(false),
                commute: Some(false),
                race: Some(false),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.id, "i450");
    assert_eq!(updated.details.intervals.len(), 1);
    assert_eq!(updated.details.interval_groups.len(), 1);
    assert_eq!(updated.details.streams.len(), 1);
}

#[tokio::test]
async fn intervals_client_updates_and_deletes_activity() {
    let server = TestIntervalsServer::start().await;
    server.set_updated_activity(ResponseActivity::sample("i404", "Updated Ride"));
    server.set_activity(ResponseActivity::sample("i404", "Updated Ride"));
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();

    let updated = client
        .update_activity(
            &credentials,
            "i404",
            UpdateActivity {
                name: Some("Updated Ride".to_string()),
                description: Some("indoors".to_string()),
                activity_type: Some("VirtualRide".to_string()),
                trainer: Some(true),
                commute: Some(false),
                race: Some(false),
            },
        )
        .await
        .unwrap();
    client.delete_activity(&credentials, "i404").await.unwrap();

    assert_eq!(updated.name.as_deref(), Some("Updated Ride"));
    assert_eq!(updated.details.streams.len(), 1);

    let requests = server.requests();
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].path, "/api/v1/activity/i404");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/api/v1/activity/i404");
    assert_eq!(requests[2].method, "GET");
    assert_eq!(requests[2].path, "/api/v1/activity/i404/intervals");
    assert_eq!(requests[3].method, "GET");
    assert_eq!(requests[3].path, "/api/v1/activity/i404/streams");
    assert_eq!(requests[4].method, "DELETE");
    assert_eq!(requests[4].path, "/api/v1/activity/i404");
}
