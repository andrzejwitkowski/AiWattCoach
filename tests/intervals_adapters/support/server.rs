use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::http::StatusCode;
use tokio::net::TcpListener;

use super::{
    fixtures::{
        ResponseActivity, ResponseActivityIntervals, ResponseActivityStream, ResponseEvent,
    },
    handlers::build_router,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CapturedRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) authorization: Option<String>,
    pub(crate) traceparent: Option<String>,
    pub(crate) body: Option<Vec<u8>>,
}

#[derive(Clone)]
pub(super) struct ServerState {
    pub(super) requests: Arc<Mutex<Vec<CapturedRequest>>>,
    pub(super) list_events: Arc<Mutex<Vec<ResponseEvent>>>,
    pub(super) list_activities: Arc<Mutex<Vec<ResponseActivity>>>,
    pub(super) list_activities_raw: Arc<Mutex<Option<serde_json::Value>>>,
    pub(super) created_event: Arc<Mutex<Option<ResponseEvent>>>,
    pub(super) updated_event: Arc<Mutex<Option<ResponseEvent>>>,
    pub(super) activity: Arc<Mutex<Option<ResponseActivity>>>,
    pub(super) activity_with_intervals: Arc<Mutex<Option<ResponseActivity>>>,
    pub(super) activity_intervals: Arc<Mutex<Option<ResponseActivityIntervals>>>,
    pub(super) activity_intervals_raw: Arc<Mutex<Option<serde_json::Value>>>,
    pub(super) activity_intervals_status: Arc<Mutex<StatusCode>>,
    pub(super) updated_activity: Arc<Mutex<Option<ResponseActivity>>>,
    pub(super) upload_ids: Arc<Mutex<Vec<String>>>,
    pub(super) streams: Arc<Mutex<Vec<ResponseActivityStream>>>,
    pub(super) streams_raw: Arc<Mutex<Option<serde_json::Value>>>,
    pub(super) streams_status: Arc<Mutex<StatusCode>>,
    pub(super) fit_bytes: Arc<Mutex<Vec<u8>>>,
    pub(super) get_status: Arc<Mutex<StatusCode>>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            list_events: Arc::new(Mutex::new(Vec::new())),
            list_activities: Arc::new(Mutex::new(Vec::new())),
            list_activities_raw: Arc::new(Mutex::new(None)),
            created_event: Arc::new(Mutex::new(None)),
            updated_event: Arc::new(Mutex::new(None)),
            activity: Arc::new(Mutex::new(None)),
            activity_with_intervals: Arc::new(Mutex::new(None)),
            activity_intervals: Arc::new(Mutex::new(None)),
            activity_intervals_raw: Arc::new(Mutex::new(None)),
            activity_intervals_status: Arc::new(Mutex::new(StatusCode::OK)),
            updated_activity: Arc::new(Mutex::new(None)),
            upload_ids: Arc::new(Mutex::new(Vec::new())),
            streams: Arc::new(Mutex::new(Vec::new())),
            streams_raw: Arc::new(Mutex::new(None)),
            streams_status: Arc::new(Mutex::new(StatusCode::OK)),
            fit_bytes: Arc::new(Mutex::new(Vec::new())),
            get_status: Arc::new(Mutex::new(StatusCode::OK)),
        }
    }
}

pub(crate) struct TestIntervalsServer {
    address: SocketAddr,
    state: ServerState,
}

impl TestIntervalsServer {
    pub(crate) async fn start() -> Self {
        let state = ServerState::default();
        let app = build_router(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { address, state }
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub(crate) fn push_event(&self, event: ResponseEvent) {
        self.state.list_events.lock().unwrap().push(event);
    }

    pub(crate) fn set_created_event(&self, event: ResponseEvent) {
        *self.state.created_event.lock().unwrap() = Some(event);
    }

    pub(crate) fn set_updated_event(&self, event: ResponseEvent) {
        *self.state.updated_event.lock().unwrap() = Some(event);
    }

    pub(crate) fn set_fit_bytes(&self, bytes: Vec<u8>) {
        *self.state.fit_bytes.lock().unwrap() = bytes;
    }

    pub(crate) fn push_activity(&self, activity: ResponseActivity) {
        self.state.list_activities.lock().unwrap().push(activity);
    }

    pub(crate) fn set_activity(&self, activity: ResponseActivity) {
        *self.state.activity.lock().unwrap() = Some(activity);
    }

    pub(crate) fn set_activity_with_intervals(&self, activity: ResponseActivity) {
        *self.state.activity_with_intervals.lock().unwrap() = Some(activity);
    }

    pub(crate) fn set_activity_intervals(&self, activity_intervals: ResponseActivityIntervals) {
        *self.state.activity_intervals.lock().unwrap() = Some(activity_intervals);
    }

    pub(crate) fn set_activity_intervals_raw(&self, payload: serde_json::Value) {
        *self.state.activity_intervals_raw.lock().unwrap() = Some(payload);
    }

    pub(crate) fn set_activity_intervals_status(&self, status: StatusCode) {
        *self.state.activity_intervals_status.lock().unwrap() = status;
    }

    pub(crate) fn set_list_activities_raw(&self, payload: serde_json::Value) {
        *self.state.list_activities_raw.lock().unwrap() = Some(payload);
    }

    pub(crate) fn set_updated_activity(&self, activity: ResponseActivity) {
        *self.state.updated_activity.lock().unwrap() = Some(activity);
    }

    pub(crate) fn set_upload_ids(&self, ids: Vec<String>) {
        *self.state.upload_ids.lock().unwrap() = ids;
    }

    pub(crate) fn set_streams(&self, streams: Vec<ResponseActivityStream>) {
        *self.state.streams.lock().unwrap() = streams;
    }

    pub(crate) fn set_streams_raw(&self, payload: serde_json::Value) {
        *self.state.streams_raw.lock().unwrap() = Some(payload);
    }

    pub(crate) fn set_streams_status(&self, status: StatusCode) {
        *self.state.streams_status.lock().unwrap() = status;
    }

    pub(crate) fn set_get_status(&self, status: StatusCode) {
        *self.state.get_status.lock().unwrap() = status;
    }

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}
