use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use aiwattcoach::{
    adapters::intervals_icu::{
        client::IntervalsIcuClient,
        settings_adapter::SettingsIntervalsProvider,
    },
    domain::{
        intervals::{
            CreateEvent, DateRange, EventCategory, IntervalsApiPort, IntervalsCredentials,
            IntervalsError, IntervalsSettingsPort, UpdateEvent,
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
    assert_eq!(requests[0].query, Some("oldest=2026-03-01&newest=2026-03-31".to_string()));
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
    assert_eq!(requests[2].path, "/api/v1/athlete/athlete-7/events/202/download.fit");
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
    created_event: Arc<Mutex<Option<ResponseEvent>>>,
    updated_event: Arc<Mutex<Option<ResponseEvent>>>,
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
            .route("/api/v1/athlete/{athlete_id}/events.json", get(list_events_handler))
            .route("/api/v1/athlete/{athlete_id}/events", post(create_event_handler))
            .route(
                "/api/v1/athlete/{athlete_id}/events/{event_id}",
                get(get_event_handler).put(update_event_handler).delete(delete_event_handler),
            )
            .route(
                "/api/v1/athlete/{athlete_id}/events/{event_id}/download.fit",
                get(download_fit_handler),
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

async fn get_event_handler(
    State(state): State<ServerState>,
    Path(path): Path<EventPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!("/api/v1/athlete/{}/events/{}", path.athlete_id, path.event_id),
        None,
        headers,
        None,
    );

    let status = *state.get_status.lock().unwrap();
    if status == StatusCode::NOT_FOUND {
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
        format!("/api/v1/athlete/{}/events/{}", path.athlete_id, path.event_id),
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
        format!("/api/v1/athlete/{}/events/{}", path.athlete_id, path.event_id),
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
