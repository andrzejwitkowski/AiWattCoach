use axum::Router;

use crate::{adapters::rest, config::AppState};

pub fn build_app(state: AppState) -> Router {
    rest::router(state)
}
