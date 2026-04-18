mod model;
mod ports;
mod service;
mod use_cases;

#[cfg(test)]
mod tests;

pub use model::{
    FtpHistoryEntry, FtpSource, TrainingLoadDailySnapshot, TrainingLoadDashboardPoint,
    TrainingLoadDashboardRange, TrainingLoadDashboardReport, TrainingLoadDashboardSummary,
    TrainingLoadError, TrainingLoadSnapshotRange, TrainingLoadTsbZone,
};
pub use ports::{
    BoxFuture, FtpHistoryRepository, NoopFtpHistoryRepository,
    NoopTrainingLoadDailySnapshotRepository, TrainingLoadDailySnapshotRepository,
};
#[cfg(test)]
pub use ports::{InMemoryFtpHistoryRepository, InMemoryTrainingLoadDailySnapshotRepository};
pub use service::build_daily_training_load_snapshots;
pub use use_cases::{
    TrainingLoadDashboardReadService, TrainingLoadDashboardReadUseCases,
    TrainingLoadRecomputeService, TrainingLoadRecomputeUseCases,
};
