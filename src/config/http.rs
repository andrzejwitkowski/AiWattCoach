use std::path::PathBuf;

use axum::Router;

use crate::{adapters::rest, config::AppState};

pub fn build_app(state: AppState) -> Router {
    build_app_with_frontend_dist(
        state,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/dist"),
    )
}

pub fn build_app_with_frontend_dist(state: AppState, frontend_dist: PathBuf) -> Router {
    rest::router_with_frontend_dist(state, frontend_dist)
}
