use std::time::Duration;

use aiwattcoach::{
    adapters::intervals_icu::client::IntervalsIcuClient,
    domain::intervals::{
        CreateEvent, DateRange, EventCategory, IntervalsApiPort, IntervalsConnectionTester,
        IntervalsCredentials, IntervalsError, UpdateEvent,
    },
};
use axum::http::StatusCode;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Instrument as _;
use tracing_subscriber::{layer::SubscriberExt, Registry};

use crate::support::{
    assert_valid_traceparent, test_credentials, ResponseEvent, TestIntervalsServer,
};

#[tokio::test]
async fn intervals_client_uses_basic_auth_and_maps_event_payloads() {
    let server = TestIntervalsServer::start().await;
    server.push_event(ResponseEvent::sample(101, "Workout 101"));
    let client = IntervalsIcuClient::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
    )
    .with_base_url(server.base_url());

    let events = client
        .list_events(
            &IntervalsCredentials {
                api_key: "secret-key".to_string(),
                athlete_id: "athlete-7".to_string(),
            },
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 101);
    assert_eq!(events[0].name.as_deref(), Some("Workout 101"));
    assert_eq!(events[0].category, EventCategory::Workout);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/athlete/athlete-7/events.json");
    assert_eq!(
        requests[0].query,
        Some("oldest=2026-03-01&newest=2026-03-31".to_string())
    );
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Basic QVBJX0tFWTpzZWNyZXQta2V5")
    );
}

#[tokio::test]
async fn intervals_connection_test_uses_api_key_basic_auth_username() {
    let server = TestIntervalsServer::start().await;
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    client
        .test_connection("secret-key", "athlete-7")
        .await
        .unwrap();

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/athlete/athlete-7");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Basic QVBJX0tFWTpzZWNyZXQta2V5")
    );
}

#[tokio::test]
async fn intervals_client_propagates_traceparent_header_from_active_span() {
    let server = TestIntervalsServer::start().await;
    server.push_event(ResponseEvent::sample(101, "Workout 101"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();
    let tracer_provider = SdkTracerProvider::builder().build();
    let tracer = tracer_provider.tracer("intervals-adapters-test");
    let subscriber = Registry::default().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let _default = tracing::subscriber::set_default(subscriber);

    let span = tracing::info_span!("intervals_client_call");
    client
        .list_events(
            &credentials,
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .instrument(span)
        .await
        .unwrap();

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_valid_traceparent(requests[0].traceparent.as_deref());
}

#[tokio::test]
async fn intervals_client_posts_updates_and_downloads_fit() {
    let server = TestIntervalsServer::start().await;
    server.set_created_event(ResponseEvent::sample(202, "Created"));
    server.set_updated_event(ResponseEvent::sample(202, "Updated"));
    server.set_fit_bytes(vec![9, 8, 7]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = test_credentials();

    let created = client
        .create_event(
            &credentials,
            CreateEvent {
                category: EventCategory::Workout,
                start_date_local: "2026-03-22".to_string(),
                name: Some("Created".to_string()),
                description: Some("desc".to_string()),
                indoor: true,
                color: Some("blue".to_string()),
                workout_doc: Some("- 5min 55%".to_string()),
                file_upload: None,
            },
        )
        .await
        .unwrap();
    let updated = client
        .update_event(
            &credentials,
            202,
            UpdateEvent {
                category: Some(EventCategory::Workout),
                start_date_local: None,
                name: Some("Updated".to_string()),
                description: None,
                indoor: Some(false),
                color: None,
                workout_doc: Some("- 2x20min".to_string()),
                file_upload: None,
            },
        )
        .await
        .unwrap();
    let fit = client.download_fit(&credentials, 202).await.unwrap();

    assert_eq!(created.id, 202);
    assert_eq!(updated.name.as_deref(), Some("Updated"));
    assert_eq!(fit, vec![9, 8, 7]);

    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[1].method, "PUT");
    assert_eq!(
        requests[2].path,
        "/api/v1/athlete/athlete-7/events/202/download.fit"
    );
}

#[tokio::test]
async fn intervals_client_maps_not_found_to_domain_error() {
    let server = TestIntervalsServer::start().await;
    server.set_get_status(StatusCode::NOT_FOUND);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let result = client.get_event(&test_credentials(), 404).await;

    assert_eq!(result, Err(IntervalsError::NotFound));
}

#[tokio::test]
async fn intervals_client_maps_upstream_auth_failures_to_credentials_error() {
    let server = TestIntervalsServer::start().await;
    server.set_get_status(StatusCode::UNAUTHORIZED);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let result = client.get_event(&test_credentials(), 401).await;

    assert_eq!(result, Err(IntervalsError::CredentialsNotConfigured));
}
