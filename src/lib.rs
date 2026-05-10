mod background_task;

#[cfg(test)]
mod test_support;

pub mod adapters;
pub mod config;
pub mod domain;
pub mod main_runtime;
pub mod telemetry;

pub use background_task::BackgroundTaskHandle;
pub use config::{
    build_app, build_app_with_frontend_dist, AppState, Settings, WhitelistRateLimiter,
};
