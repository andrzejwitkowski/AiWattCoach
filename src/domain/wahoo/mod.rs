mod import_mapping;
mod model;
mod plan_mapping;
mod ports;
mod service;
#[cfg(test)]
mod tests;
mod webhook;

pub const MISSING_WAHOO_FTP_MESSAGE: &str =
    "Set your cycling FTP in Settings before syncing to Wahoo";

pub use import_mapping::map_workout_to_import_command;
pub use model::{
    WahooAuthExchange, WahooAuthStart, WahooConnectState, WahooCreatePlan, WahooCreateWorkout,
    WahooError, WahooFileReference, WahooPlan, WahooToken, WahooUpdatePlan, WahooUpdateWorkout,
    WahooUser, WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
};
pub use plan_mapping::build_plan_file_json;
pub use ports::{BoxFuture, WahooApiPort, WahooConnectStateRepository, WahooOAuthPort};
pub use service::{WahooService, WahooUseCases};
pub use webhook::{
    ManualWahooSyncResult, WahooWebhookAccepted, WahooWebhookError, WahooWebhookOutcome,
    WahooWebhookService, WahooWebhookUseCases,
};
