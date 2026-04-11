use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use super::{
    fixtures::{
        ResponseActivity, ResponseActivityId, ResponseActivityIntervals, ResponseEvent,
        ResponseUpload,
    },
    server::{CapturedRequest, ServerState},
};

pub(super) fn build_router(state: ServerState) -> Router {
    Router::new()
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
            "/api/v1/activity/{activity_id}/intervals",
            get(get_activity_intervals_handler),
        )
        .route(
            "/api/v1/activity/{activity_id}/streams",
            get(get_activity_streams_handler),
        )
        .with_state(state)
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
    if let Some(payload) = state.list_events_raw.lock().unwrap().clone() {
        return Json(payload).into_response();
    }

    Json(state.list_events.lock().unwrap().clone()).into_response()
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
        Some(body.to_string().into_bytes()),
    );

    if let Some((status, payload)) = state.created_event_failure.lock().unwrap().clone() {
        return (status, Json(payload)).into_response();
    }

    Json(
        state
            .created_event
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseEvent::sample(1, "Created")),
    )
    .into_response()
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
        Some(body.to_string().into_bytes()),
    );

    if let Some((status, payload)) = state.updated_event_failure.lock().unwrap().clone() {
        return (status, Json(payload)).into_response();
    }

    Json(
        state
            .updated_event
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ResponseEvent::sample(path.event_id, "Updated")),
    )
    .into_response()
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

    let status = *state.list_activities_status.lock().unwrap();

    if let Some(payload) = state.list_activities_raw.lock().unwrap().clone() {
        return (status, Json(payload)).into_response();
    }

    if status != StatusCode::OK {
        return status.into_response();
    }

    Json(state.list_activities.lock().unwrap().clone()).into_response()
}

async fn get_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let include_intervals = query.get("intervals").is_some_and(|value| value == "true");
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

    let activity = if include_intervals {
        state
            .activity_with_intervals
            .lock()
            .unwrap()
            .clone()
            .or_else(|| state.activity.lock().unwrap().clone())
    } else {
        state.activity.lock().unwrap().clone()
    };

    Json(activity.unwrap_or_else(|| ResponseActivity::sample(&path.activity_id, "Activity")))
}

async fn get_activity_streams_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    Query(query): Query<HashMap<String, String>>,
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
    let status = *state.streams_status.lock().unwrap();
    if status != StatusCode::OK {
        return status.into_response();
    }

    if let Some(payload) = state.streams_raw.lock().unwrap().clone() {
        return Json(payload).into_response();
    }

    Json(state.streams.lock().unwrap().clone()).into_response()
}

async fn get_activity_intervals_handler(
    State(state): State<ServerState>,
    Path(path): Path<ActivityPath>,
    headers: HeaderMap,
) -> impl IntoResponse {
    capture_request(
        &state,
        "GET",
        format!("/api/v1/activity/{}/intervals", path.activity_id),
        None,
        headers,
        None,
    );

    let status = *state.activity_intervals_status.lock().unwrap();
    if status != StatusCode::OK {
        return status.into_response();
    }

    if let Some(payload) = state.activity_intervals_raw.lock().unwrap().clone() {
        return Json(payload).into_response();
    }

    Json(
        state
            .activity_intervals
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(ResponseActivityIntervals::empty),
    )
    .into_response()
}

async fn upload_activity_handler(
    State(state): State<ServerState>,
    Path(path): Path<AthletePath>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
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
        Some(body.to_vec()),
    );

    if let Some((status, payload)) = state.upload_failure.lock().unwrap().clone() {
        return (status, Json(payload)).into_response();
    }

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
        .into_response()
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
        Some(body.to_string().into_bytes()),
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
    body: Option<Vec<u8>>,
) {
    state.requests.lock().unwrap().push(CapturedRequest {
        method: method.to_string(),
        path,
        query,
        authorization: headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        traceparent: headers
            .get("traceparent")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        body,
    });
}
