use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use aiwattcoach::{
    adapters::intervals_icu::{
        client::IntervalsIcuClient, settings_adapter::SettingsIntervalsProvider,
    },
    domain::{
        intervals::{
            CreateEvent, DateRange, EventCategory, IntervalsApiPort, IntervalsConnectionTester,
            IntervalsCredentials, IntervalsError, IntervalsSettingsPort, UpdateActivity,
            UpdateEvent, UploadActivity,
        },
        settings::{
            AiAgentsConfig, AnalysisOptions, CyclingSettings, IntervalsConfig, SettingsError,
            UserSettings, UserSettingsUseCases,
        },
    },
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[tokio::test]
async fn settings_provider_returns_credentials_from_user_settings() {
    let settings_service = Arc::new(FakeSettingsUseCases::with_intervals(IntervalsConfig {
        api_key: Some("key-123".to_string()),
        athlete_id: Some("athlete-99".to_string()),
        connected: true,
    }));
    let provider = SettingsIntervalsProvider::new(settings_service);

    let credentials = provider.get_credentials("user-1").await.unwrap();

    assert_eq!(
        credentials,
        IntervalsCredentials {
            api_key: "key-123".to_string(),
            athlete_id: "athlete-99".to_string(),
        }
    );
}

#[tokio::test]
async fn settings_provider_rejects_missing_credentials() {
    let settings_service = Arc::new(FakeSettingsUseCases::with_intervals(IntervalsConfig {
        api_key: None,
        athlete_id: Some("athlete-99".to_string()),
        connected: false,
    }));
    let provider = SettingsIntervalsProvider::new(settings_service);

    let result = provider.get_credentials("user-1").await;

    assert_eq!(result, Err(IntervalsError::CredentialsNotConfigured));
}

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
async fn intervals_client_posts_updates_and_downloads_fit() {
    let server = TestIntervalsServer::start().await;
    server.set_created_event(ResponseEvent::sample(202, "Created"));
    server.set_updated_event(ResponseEvent::sample(202, "Updated"));
    server.set_fit_bytes(vec![9, 8, 7]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = IntervalsCredentials {
        api_key: "secret-key".to_string(),
        athlete_id: "athlete-7".to_string(),
    };

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

    let result = client
        .get_event(
            &IntervalsCredentials {
                api_key: "secret-key".to_string(),
                athlete_id: "athlete-7".to_string(),
            },
            404,
        )
        .await;

    assert_eq!(result, Err(IntervalsError::NotFound));
}

#[tokio::test]
async fn intervals_client_maps_upstream_auth_failures_to_credentials_error() {
    let server = TestIntervalsServer::start().await;
    server.set_get_status(StatusCode::UNAUTHORIZED);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let result = client
        .get_event(
            &IntervalsCredentials {
                api_key: "secret-key".to_string(),
                athlete_id: "athlete-7".to_string(),
            },
            401,
        )
        .await;

    assert_eq!(result, Err(IntervalsError::CredentialsNotConfigured));
}

#[tokio::test]
async fn intervals_client_lists_activities_and_normalizes_metrics() {
    let server = TestIntervalsServer::start().await;
    server.push_activity(ResponseActivity::sample("i101", "Tempo Ride"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activities = client
        .list_activities(
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
async fn intervals_client_gets_activity_with_intervals_and_streams() {
    let server = TestIntervalsServer::start().await;
    server.set_activity(ResponseActivity::sample("i202", "Loaded Ride"));
    server.set_streams(vec![ResponseActivityStream::sample_watts()]);
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());

    let activity = client
        .get_activity(
            &IntervalsCredentials {
                api_key: "secret-key".to_string(),
                athlete_id: "athlete-7".to_string(),
            },
            "i202",
        )
        .await
        .unwrap();

    assert_eq!(activity.id, "i202");
    assert_eq!(activity.details.intervals.len(), 1);
    assert_eq!(activity.details.streams.len(), 1);
    assert_eq!(activity.details.streams[0].stream_type, "watts");

    let requests = server.requests();
    assert_eq!(requests[0].path, "/api/v1/activity/i202");
    assert_eq!(requests[0].query.as_deref(), Some("intervals=true"));
    assert_eq!(requests[1].path, "/api/v1/activity/i202/streams");
}

#[tokio::test]
async fn intervals_client_uploads_activity_and_fetches_uploaded_details() {
    let server = TestIntervalsServer::start().await;
    server.set_upload_ids(vec!["i303".to_string()]);
    server.set_activity(ResponseActivity::sample("i303", "Uploaded Ride"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = IntervalsCredentials {
        api_key: "secret-key".to_string(),
        athlete_id: "athlete-7".to_string(),
    };

    let result = client
        .upload_activity(
            &credentials,
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3, 4],
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
}

#[tokio::test]
async fn intervals_client_updates_and_deletes_activity() {
    let server = TestIntervalsServer::start().await;
    server.set_updated_activity(ResponseActivity::sample("i404", "Updated Ride"));
    let client = IntervalsIcuClient::new(reqwest::Client::new()).with_base_url(server.base_url());
    let credentials = IntervalsCredentials {
        api_key: "secret-key".to_string(),
        athlete_id: "athlete-7".to_string(),
    };

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

    let requests = server.requests();
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].path, "/api/v1/activity/i404");
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].path, "/api/v1/activity/i404");
}

#[derive(Clone)]
struct FakeSettingsUseCases {
    settings: UserSettings,
}

impl FakeSettingsUseCases {
    fn with_intervals(intervals: IntervalsConfig) -> Self {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
        settings.intervals = intervals;
        Self { settings }
    }
}

impl UserSettingsUseCases for FakeSettingsUseCases {
    fn get_settings(&self, _user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ResponseEvent {
    id: i64,
    start_date_local: String,
    name: Option<String>,
    category: String,
    description: Option<String>,
    indoor: Option<bool>,
    color: Option<String>,
    workout_doc: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ResponseActivity {
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
    trainer: Option<bool>,
    commute: Option<bool>,
    race: Option<bool>,
    has_heartrate: Option<bool>,
    stream_types: Option<Vec<String>>,
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
    fn sample(id: &str, name: &str) -> Self {
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
            icu_intervals: Some(vec![ResponseActivityInterval::sample()]),
            icu_groups: Some(vec![ResponseActivityGroup::sample()]),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ResponseActivityInterval {
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
    fn sample() -> Self {
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
struct ResponseActivityGroup {
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
    fn sample() -> Self {
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
struct ResponseActivityStream {
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
    fn sample_watts() -> Self {
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
struct ResponseUpload {
    activities: Vec<ResponseActivityId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ResponseActivityId {
    id: String,
}

impl ResponseEvent {
    fn sample(id: i64, name: &str) -> Self {
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CapturedRequest {
    method: String,
    path: String,
    query: Option<String>,
    authorization: Option<String>,
    body: Option<String>,
}

#[derive(Clone, Default)]
struct ServerState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    list_events: Arc<Mutex<Vec<ResponseEvent>>>,
    list_activities: Arc<Mutex<Vec<ResponseActivity>>>,
    created_event: Arc<Mutex<Option<ResponseEvent>>>,
    updated_event: Arc<Mutex<Option<ResponseEvent>>>,
    activity: Arc<Mutex<Option<ResponseActivity>>>,
    updated_activity: Arc<Mutex<Option<ResponseActivity>>>,
    upload_ids: Arc<Mutex<Vec<String>>>,
    streams: Arc<Mutex<Vec<ResponseActivityStream>>>,
    fit_bytes: Arc<Mutex<Vec<u8>>>,
    get_status: Arc<Mutex<StatusCode>>,
}

struct TestIntervalsServer {
    address: SocketAddr,
    state: ServerState,
}

impl TestIntervalsServer {
    async fn start() -> Self {
        let state = ServerState::default();
        let app = Router::new()
            .route("/api/v1/athlete/{athlete_id}", get(test_connection_handler))
            .route(
                "/api/v1/athlete/{athlete_id}/events.json",
                get(list_events_handler),
            )
            .route(
                "/api/v1/athlete/{athlete_id}/events",
                post(create_event_handler),
            )
            .route(
                "/api/v1/athlete/{athlete_id}/events/{event_id}",
                get(get_event_handler)
                    .put(update_event_handler)
                    .delete(delete_event_handler),
            )
            .route(
                "/api/v1/athlete/{athlete_id}/events/{event_id}/download.fit",
                get(download_fit_handler),
            )
            .route(
                "/api/v1/athlete/{athlete_id}/activities",
                get(list_activities_handler).post(upload_activity_handler),
            )
            .route(
                "/api/v1/activity/{activity_id}",
                get(get_activity_handler)
                    .put(update_activity_handler)
                    .delete(delete_activity_handler),
            )
            .route(
                "/api/v1/activity/{activity_id}/streams",
                get(get_activity_streams_handler),
            )
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { address, state }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    fn push_event(&self, event: ResponseEvent) {
        self.state.list_events.lock().unwrap().push(event);
    }

    fn set_created_event(&self, event: ResponseEvent) {
        *self.state.created_event.lock().unwrap() = Some(event);
    }

    fn set_updated_event(&self, event: ResponseEvent) {
        *self.state.updated_event.lock().unwrap() = Some(event);
    }

    fn set_fit_bytes(&self, bytes: Vec<u8>) {
        *self.state.fit_bytes.lock().unwrap() = bytes;
    }

    fn push_activity(&self, activity: ResponseActivity) {
        self.state.list_activities.lock().unwrap().push(activity);
    }

    fn set_activity(&self, activity: ResponseActivity) {
        *self.state.activity.lock().unwrap() = Some(activity);
    }

    fn set_updated_activity(&self, activity: ResponseActivity) {
        *self.state.updated_activity.lock().unwrap() = Some(activity);
    }

    fn set_upload_ids(&self, ids: Vec<String>) {
        *self.state.upload_ids.lock().unwrap() = ids;
    }

    fn set_streams(&self, streams: Vec<ResponseActivityStream>) {
        *self.state.streams.lock().unwrap() = streams;
    }

    fn set_get_status(&self, status: StatusCode) {
        *self.state.get_status.lock().unwrap() = status;
    }

    fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

#[derive(Deserialize)]
struct EventQuery {
    oldest: String,
    newest: String,
}

#[derive(Deserialize)]
struct EventPath {
    athlete_id: String,
    event_id: i64,
}

#[derive(Deserialize)]
struct AthletePath {
    athlete_id: String,
}

#[derive(Deserialize)]
struct ActivityPath {
    activity_id: String,
}

async fn list_events_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    Query(query): Query<EventQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!("/api/v1/athlete/{}/events.json", path.athlete_id),
        Some(format!("oldest={}&newest={}", query.oldest, query.newest)),
        headers,
        None,
    );
    Json(state.list_events.lock().unwrap().clone())
}

async fn test_connection_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!("/api/v1/athlete/{}", path.athlete_id),
        None,
        headers,
        None,
    );

    let status = *state.get_status.lock().unwrap();
    if status != StatusCode::OK {
        return status.into_response();
    }

    StatusCode::OK.into_response()
}

async fn get_event_handler(
    State(state): State<ServerState>,
    Path(path): Path<EventPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!(
            "/api/v1/athlete/{}/events/{}",
            path.athlete_id, path.event_id
        ),
        None,
        headers,
        None,
    );

    let status = *state.get_status.lock().unwrap();
    if status != StatusCode::OK {
        return status.into_response();
    }

    Json(ResponseEvent::sample(path.event_id, "Fetched")).into_response()
}

async fn create_event_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    capture_request(
        &state,
        "POST",
        format!("/api/v1/athlete/{}/events", path.athlete_id),
        None,
        headers,
        Some(body.to_string()),
    );

    Json(
        state
            .created_event
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseEvent::sample(1, "Created")),
    )
}

async fn update_event_handler(
    State(state): State<ServerState>,
    Path(path): Path<EventPath>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    capture_request(
        &state,
        "PUT",
        format!(
            "/api/v1/athlete/{}/events/{}",
            path.athlete_id, path.event_id
        ),
        None,
        headers,
        Some(body.to_string()),
    );

    Json(
        state
            .updated_event
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseEvent::sample(path.event_id, "Updated")),
    )
}

async fn delete_event_handler(
    State(state): State<ServerState>,
    Path(path): Path<EventPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "DELETE",
        format!(
            "/api/v1/athlete/{}/events/{}",
            path.athlete_id, path.event_id
        ),
        None,
        headers,
        None,
    );
    StatusCode::NO_CONTENT
}

async fn download_fit_handler(
    State(state): State<ServerState>,
    Path(path): Path<EventPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!(
            "/api/v1/athlete/{}/events/{}/download.fit",
            path.athlete_id, path.event_id
        ),
        None,
        headers,
        None,
    );
    (
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from(state.fit_bytes.lock().unwrap().clone()),
    )
}

async fn list_activities_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    Query(query): Query<EventQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!("/api/v1/athlete/{}/activities", path.athlete_id),
        Some(format!("oldest={}&newest={}", query.oldest, query.newest)),
        headers,
        None,
    );
    Json(state.list_activities.lock().unwrap().clone())
}

async fn get_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    Query(query): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    capture_request(
        &state,
        "GET",
        format!("/api/v1/activity/{}", path.activity_id),
        if query_string.is_empty() {
            None
        } else {
            Some(query_string)
        },
        headers,
        None,
    );

    Json(
        state
            .activity
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseActivity::sample(&path.activity_id, "Activity")),
    )
}

async fn get_activity_streams_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    Query(query): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    capture_request(
        &state,
        "GET",
        format!("/api/v1/activity/{}/streams", path.activity_id),
        if query_string.is_empty() {
            None
        } else {
            Some(query_string)
        },
        headers,
        None,
    );
    Json(state.streams.lock().unwrap().clone())
}

async fn upload_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    Query(query): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let query_string = query
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    capture_request(
        &state,
        "POST",
        format!("/api/v1/athlete/{}/activities", path.athlete_id),
        if query_string.is_empty() {
            None
        } else {
            Some(query_string)
        },
        headers,
        Some(body),
    );
    let ids = state.upload_ids.lock().unwrap().clone();
    (
        StatusCode::CREATED,
        Json(ResponseUpload {
            activities: ids
                .into_iter()
                .map(|id| ResponseActivityId { id })
                .collect(),
        }),
    )
}

async fn update_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    capture_request(
        &state,
        "PUT",
        format!("/api/v1/activity/{}", path.activity_id),
        None,
        headers,
        Some(body.to_string()),
    );
    Json(
        state
            .updated_activity
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseActivity::sample(&path.activity_id, "Updated Activity")),
    )
}

async fn delete_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "DELETE",
        format!("/api/v1/activity/{}", path.activity_id),
        None,
        headers,
        None,
    );
    StatusCode::NO_CONTENT
}

fn capture_request(
    state: &ServerState,
    method: &str,
    path: String,
    query: Option<String>,
    headers: HeaderMap,
    body: Option<String>,
) {
    state.requests.lock().unwrap().push(CapturedRequest {
        method: method.to_string(),
        path,
        query,
        authorization: headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        body,
    });
}
