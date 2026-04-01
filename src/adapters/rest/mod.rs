mod admin;
mod auth;
mod cookies;
mod health;
mod intervals;
mod logging;
mod logs;
mod settings;
mod user_auth;
mod workout_summary;

use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, MatchedPath, Request},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
    routing::{get, patch, post},
    Router,
};
use opentelemetry::{
    propagation::TextMapPropagator,
    trace::{SpanContext, SpanId, TraceContextExt as _, TraceFlags, TraceId, TraceState},
    Context,
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tower::util::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::{field::Empty, Level, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::config::AppState;

use self::logging::status_class;

pub fn router(state: AppState) -> Router {
    router_with_frontend_dist(
        state,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/dist"),
    )
}

pub fn router_with_frontend_dist(state: AppState, frontend_dist: PathBuf) -> Router {
    let static_files = ServeDir::new(&frontend_dist);
    let spa_index = ServeFile::new(frontend_dist.join("index.html"));

    Router::new()
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route("/api/auth/google/start", get(auth::start_google_login))
        .route("/api/auth/google/callback", get(auth::finish_google_login))
        .route("/api/auth/me", get(auth::current_user))
        .route("/api/auth/logout", post(auth::logout))
        .route(
            "/api/logs",
            post(logs::ingest_logs).layer(DefaultBodyLimit::max(logs::MAX_REQUEST_BODY_BYTES)),
        )
        .route("/api/admin/system-info", get(admin::system_info))
        .route(
            "/api/admin/settings/{user_id}",
            get(settings::admin_get_user_settings),
        )
        .route("/api/settings", get(settings::get_settings))
        .route("/api/settings/ai-agents", patch(settings::update_ai_agents))
        .route("/api/settings/intervals", patch(settings::update_intervals))
        .route(
            "/api/settings/intervals/test",
            post(settings::test_intervals_connection),
        )
        .route("/api/settings/options", patch(settings::update_options))
        .route("/api/settings/cycling", patch(settings::update_cycling))
        .route(
            "/api/workout-summaries",
            get(workout_summary::list_summaries),
        )
        .route(
            "/api/workout-summaries/{workout_id}",
            get(workout_summary::get_summary).post(workout_summary::create_summary),
        )
        .route(
            "/api/workout-summaries/{workout_id}/rpe",
            patch(workout_summary::update_rpe),
        )
        .route(
            "/api/workout-summaries/{workout_id}/state",
            post(workout_summary::set_saved_state).patch(workout_summary::set_saved_state),
        )
        .route(
            "/api/workout-summaries/{workout_id}/messages",
            post(workout_summary::send_message),
        )
        .route(
            "/api/workout-summaries/{workout_id}/ws",
            get(workout_summary::workout_summary_ws),
        )
        .route(
            "/api/intervals/events",
            get(intervals::list_events).post(intervals::create_event),
        )
        .route(
            "/api/intervals/events/{event_id}",
            get(intervals::get_event)
                .put(intervals::update_event)
                .delete(intervals::delete_event),
        )
        .route(
            "/api/intervals/events/{event_id}/download.fit",
            get(intervals::download_fit),
        )
        .route(
            "/api/intervals/activities",
            get(intervals::list_activities)
                .post(intervals::create_activity)
                .layer(DefaultBodyLimit::max(
                    intervals::MAX_ACTIVITY_UPLOAD_REQUEST_BYTES,
                )),
        )
        .route(
            "/api/intervals/activities/{activity_id}",
            get(intervals::get_activity)
                .put(intervals::update_activity)
                .delete(intervals::delete_activity),
        )
        .fallback(move |request| serve_frontend(request, static_files.clone(), spa_index.clone()))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(make_request_span)
                .on_response(log_response_event),
        )
        .with_state(state)
}

fn make_request_span(request: &Request) -> Span {
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str);
    let route = matched_path.unwrap_or_else(|| request.uri().path());
    let span = tracing::info_span!(
        "http_request",
        http.method = %request.method(),
        http.route = %route,
        http.target = %request.uri().path(),
        http.status_code = Empty,
        user_id = Empty,
        trace_id = Empty,
    );

    apply_incoming_trace_context(request.headers(), &span);
    span
}

fn apply_incoming_trace_context(headers: &HeaderMap, span: &Span) {
    let parent_context = TraceContextPropagator::new().extract(&HeaderExtractor(headers));
    let incoming_trace_id = {
        let parent_span = parent_context.span();
        let parent_span_context = parent_span.span_context();

        parent_span_context
            .is_valid()
            .then(|| parent_span_context.trace_id().to_string())
    };

    if let Some(trace_id) = incoming_trace_id {
        span.set_parent(parent_context);
        span.record("trace_id", tracing::field::display(trace_id));
        return;
    }

    let generated_trace_id = TraceId::from_hex(&uuid::Uuid::new_v4().to_string().replace('-', ""))
        .expect("uuid v4 without dashes should be a valid 32-char hex trace id");
    let generated_parent = synthetic_remote_parent_context(generated_trace_id);
    span.set_parent(generated_parent);
    span.record(
        "trace_id",
        tracing::field::display(generated_trace_id.to_string()),
    );
}

fn synthetic_remote_parent_context(trace_id: TraceId) -> Context {
    let span_id = SpanId::from_hex(&uuid::Uuid::new_v4().to_string().replace('-', "")[..16])
        .expect("uuid v4 prefix should be a valid 16-char hex span id");

    Context::new().with_remote_span_context(SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    ))
}

fn record_span_trace_id(span: &Span) {
    let span_context = span.context().span().span_context().clone();

    if span_context.is_valid() {
        span.record(
            "trace_id",
            tracing::field::display(span_context.trace_id().to_string()),
        );
    }
}

fn log_response_event<B>(response: &Response<B>, latency: std::time::Duration, span: &Span) {
    let status = response.status();
    let status_class = status_class(status);

    record_span_trace_id(span);
    span.record("http.status_code", status.as_u16());

    let _guard = span.enter();

    match status_level(status) {
        Level::ERROR => tracing::event!(
            Level::ERROR,
            status = status.as_u16(),
            status_class,
            latency_ms = latency.as_millis(),
            "finished processing request"
        ),
        Level::WARN => tracing::event!(
            Level::WARN,
            status = status.as_u16(),
            status_class,
            latency_ms = latency.as_millis(),
            "finished processing request"
        ),
        _ => tracing::event!(
            Level::INFO,
            status = status.as_u16(),
            status_class,
            latency_ms = latency.as_millis(),
            "finished processing request"
        ),
    }
}

fn status_level(status: StatusCode) -> Level {
    if status.is_server_error() {
        Level::ERROR
    } else if status.is_client_error() {
        Level::WARN
    } else {
        Level::INFO
    }
}

async fn serve_frontend(
    request: Request,
    static_files: ServeDir,
    spa_index: ServeFile,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path();
    let can_fall_back_to_spa = !is_api_route(path)
        && matches!(method, Method::GET | Method::HEAD)
        && accepts_html(request.headers(), path);

    if is_api_route(path) {
        return not_found_response();
    }

    if !matches!(method, Method::GET | Method::HEAD) {
        return not_found_response();
    }

    let response = static_files.oneshot(request).await;

    match response {
        Ok(response) if response.status() != StatusCode::NOT_FOUND => response.map(Body::new),
        Ok(_) if can_fall_back_to_spa => match spa_index
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
        {
            Ok(response) => response.map(Body::new),
            Err(_) => internal_error_response(),
        },
        Ok(response) => response.map(Body::new),
        Err(_) => internal_error_response(),
    }
}

fn is_api_route(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn accepts_html(headers: &HeaderMap, path: &str) -> bool {
    !is_file_like_path(path)
        && (path == "/"
            || headers
                .get(header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.contains("text/html") || value.contains("application/xhtml+xml"))
                .unwrap_or(false))
}

fn is_file_like_path(path: &str) -> bool {
    let last_segment = path.rsplit('/').next().unwrap_or_default();
    let extension = last_segment
        .rsplit_once('.')
        .map(|(_, extension)| extension);

    path.starts_with("/assets/")
        || path.starts_with("/.well-known/")
        || matches!(last_segment, "apple-app-site-association")
        || matches!(
            extension,
            Some(
                "txt"
                    | "htm"
                    | "html"
                    | "webmanifest"
                    | "ico"
                    | "json"
                    | "js"
                    | "css"
                    | "map"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "svg"
                    | "webp"
                    | "gif"
                    | "woff"
                    | "woff2"
                    | "ttf"
            )
        )
}

fn not_found_response() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from("not found"))
        .unwrap()
}

fn internal_error_response() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from("failed to serve frontend asset"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt as _, TraceId};

    use super::synthetic_remote_parent_context;

    #[test]
    fn synthetic_remote_parent_context_is_valid_and_sampled() {
        let trace_id = TraceId::from_hex("0af7651916cd43dd8448eb211c80319c").unwrap();
        let context = synthetic_remote_parent_context(trace_id);
        let span_context: SpanContext = context.span().span_context().clone();

        assert!(span_context.is_remote());
        assert!(span_context.is_valid());
        assert!(span_context.trace_flags().is_sampled());
        assert_eq!(span_context.trace_id(), trace_id);
        assert_ne!(span_context.span_id(), SpanId::INVALID);
    }
}
