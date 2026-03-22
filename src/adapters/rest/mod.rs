mod admin;
mod auth;
mod cookies;
mod health;
mod settings;

use std::path::PathBuf;

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
    routing::{get, patch, post},
    Router,
};
use tower::util::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::config::AppState;

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
        .route("/api/admin/system-info", get(admin::system_info))
        .route("/api/settings", get(settings::get_settings))
        .route("/api/settings/ai-agents", patch(settings::update_ai_agents))
        .route("/api/settings/intervals", patch(settings::update_intervals))
        .route("/api/settings/options", patch(settings::update_options))
        .route("/api/settings/cycling", patch(settings::update_cycling))
        .fallback(move |request| serve_frontend(request, static_files.clone(), spa_index.clone()))
        .with_state(state)
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
